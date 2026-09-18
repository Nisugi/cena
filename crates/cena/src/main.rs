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
//! `cena-platform`'s [`LiveSource`](cena_platform::LiveSource) and
//! [`eaccess`](cena_platform::eaccess) both carry a BUILT, NOT RUN header
//! saying so. Criterion 1 is met by **the author running this**, watching it,
//! and nothing else.
//!
//! So the output is written to be *read by a person who is deciding whether it
//! passed*. Every stage announces itself; the room is printed from typed
//! frames; the behavior's commands and the manual one are both visible in the
//! order the wire saw them.
//!
//! # Credentials are prompted, never stored
//!
//! `plan/12` §7.1 puts "saved credentials, GUI login, web-login fallback"
//! explicitly **Out** for Milestone 1. This prompts on stdin, keeps the
//! password only as long as the handshake needs it, and writes nothing to
//! disk. It is never logged, and the types that hold it redact themselves
//! (`cena_platform::Credentials`'s `Debug`).
//!
//! **The prompt ECHOES what you type.** There is no terminal echo suppression
//! here: the password appears on screen as it is typed and stays in the
//! terminal's scrollback. Suppressing it needs `rpassword` or raw-mode
//! handling, which is the credential ladder `plan/12` §7.1 puts Out for M1.
//!
//! This paragraph replaced the claim "the password is never printed", which
//! was **false** and was caught by adversarial review. Do not run this on a
//! shared screen or a recorded session until the prompt is fixed.
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

use ask::ask;
use cena_behavior::{is_room_description, look};
use cena_platform::{ByteSource, Credentials, Redactions, SessionSink};
use cena_session::{AuthorityToken, CommandId, Event, Frame, Origin, Session, SessionHandle};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// How long to let the behavior run before the manual command interleaves.
const BEHAVIOR_WARMUP: Duration = Duration::from_secs(3);

/// How long the whole demonstration runs before `stop`.
const RUN_FOR: Duration = Duration::from_secs(10);

/// How long to wait for the first room description after login.
///
/// The login burst carries the room unprompted (MEASURED 2026-09-18: 2,347
/// bytes including room, exits and inventory), so this is generous rather than
/// tight -- a slow link should not look like a protocol failure.
const ROOM_DEADLINE: Duration = Duration::from_secs(20);

/// A monotonic `CommandId` source, seeded at 0 -- never from a clock or a
/// random, so a recording replays to the same ids (criterion 7).
fn ids() -> impl FnMut() -> CommandId {
    let next = Arc::new(AtomicU64::new(0));
    move || CommandId(next.fetch_add(1, Ordering::Relaxed))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("cena -- Milestone 1 Step 2, the end-to-end slice.");
    eprintln!("Nothing is written to disk; the password is not stored or logged.\n");

    let typed = ask()?;
    eprintln!();

    // --- Criterion 1: log in against the live server ------------------------
    let creds = Credentials {
        account: &typed.account,
        password: &typed.password,
        character: &typed.character,
        game_code: &typed.game_code,
    };
    // Kept past `drop(typed)` below: the log filename needs it, and the
    // character name is not a credential.
    let typed_character = typed.character.clone();
    let payload = cena_platform::authenticate(creds, |line| eprintln!("{line}")).await?;
    // The credentials are finished with: the launch key replaces them, and the
    // game socket never sees the password. Dropped here rather than at the end
    // of `main` so it is not live across the whole game session.
    //
    // **This is not a security measure and must not be read as one.** Rust
    // does not zero a dropped `String`, so the bytes stay in freed heap memory
    // until something reuses the allocation. Zeroing on drop needs a type that
    // does it (`zeroize`), which `plan/12` §7.1 puts Out of scope for M1 along
    // with the rest of the credential ladder. Recorded so the gap is visible
    // rather than assumed closed.
    drop(typed);
    eprintln!("\n[login] {payload:?}");

    let socket = cena_platform::connect_game(&payload).await?;
    eprintln!("[login] game socket open, WRAYTH banner and ready signals sent\n");

    // --- The session owns the socket from here -----------------------------
    let session = open_session(socket, &typed_character, &payload);
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let (_snapshot, mut events) = session.subscribe();
    let actor = tokio::spawn(session.into_actor().run());

    // --- Criterion 2: the room, from TYPED FRAMES --------------------------
    eprintln!("[waiting] for the first room description frame...");
    let room_shown = wait_for_room(&mut events).await;

    if !room_shown {
        eprintln!(
            "[FAIL criterion 2] no room description frame within {ROOM_DEADLINE:?}. \
             The session is connected -- 'connected' is not 'working' (plan/10 \
             §10.3a). Check the WRAYTH banner reached the server."
        );
    }

    // --- Criteria 3-5: the behavior, and a command interleaved with it -----
    // The behavior and the manual surface hold the SAME handle type, cloned
    // from the same session. Criterion 3's "the same queue as the behavior's"
    // is structural here, not asserted.
    let behavior_handle = handle.clone();
    let stop = CancellationToken::new();
    let behavior_stop = stop.clone();
    eprintln!("\n[behavior] starting `look`, interval 1s. Watch for interleaving.\n");
    let behavior = tokio::spawn(async move {
        look(&behavior_handle, &behavior_stop, ids(), AuthorityToken(1)).await
    });

    // Print what crosses the wire, both directions, in order. This is what
    // makes criterion 5 visible rather than merely true.
    let watcher = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(Event::Sent { line, origin }) => {
                    let tag = match origin {
                        Origin::Manual => "manual",
                        Origin::Behavior(_) => "behavior",
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
    });

    tokio::time::sleep(BEHAVIOR_WARMUP).await;

    // Criterion 5: a manual command MID-BEHAVIOR. It jumps the queue, runs its
    // round trip, and the behavior CONTINUES -- only an explicit stop preempts
    // (`plan/12` §4.1, corrected 2026-09-18).
    eprintln!("\n[manual] typing `look` mid-behavior -- the behavior must continue\n");
    let outcome = send_manual(&handle, "look").await;
    eprintln!("\n[manual] outcome: {outcome:?}");
    eprintln!(
        "[criterion 5] behavior still running: {}\n",
        !behavior.is_finished()
    );

    tokio::time::sleep(RUN_FOR).await;

    // --- Criterion 4: stop, within PREEMPT_GRACE ---------------------------
    // The latency is MEASURED in `cena-behavior`'s tests under virtual time,
    // where it is a property of the code rather than of this machine's load.
    // Here it is only demonstrated.
    eprintln!("\n[stop] cancelling the behavior");
    let at_stop = std::time::Instant::now();
    stop.cancel();
    let result = behavior.await?;
    eprintln!(
        "[stop] behavior ended in {:?}: {result:?}",
        at_stop.elapsed()
    );

    // --- Criterion 6: clean disconnect -------------------------------------
    eprintln!("\n[disconnect] cancelling the session");
    session_cancel.cancel();
    // NOT `actor.await?`. `plan/12` §5.5 says "a panic kills one session, not
    // the process", and criterion 6's evidence is the shutdown report below --
    // so a panicking actor is exactly the case where that report matters most,
    // and `?` here would skip it and hide the panic inside a `JoinError`.
    let joined = actor.await;
    watcher.abort();

    match joined {
        Ok(mut end) => {
            // "No leaked sockets" is checked by ASKING THE SOURCE, not by
            // trusting a drop ran. A second shutdown is idempotent by the
            // trait's contract.
            let shutdown = end.source.shutdown().await;
            eprintln!(
                "[disconnect] lifecycle={:?}, shutdown={:?}, {} events recorded",
                end.lifecycle,
                shutdown.is_ok(),
                end.recorder.events().len()
            );
            eprintln!("\nDone. Criteria 1-6 exercised against the live server.");
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

/// Build the session, attaching a log unless one cannot be opened.
///
/// Split from `main` under `plan/05` Rule 4.1 -- move code down, do not raise
/// the cap -- when clippy caught `main` at 112 lines against a 100 limit.
fn open_session(
    socket: cena_platform::LiveSource,
    character: &str,
    payload: &cena_platform::LaunchPayload,
) -> Session<cena_platform::LiveSource> {
    // Logging is ON by default. Author's call, 2026-09-18: "we want it on by
    // default during our dev work. That way there's always a log for you."
    //
    // Opt-OUT, not opt-in: a session that fails in an interesting way is
    // exactly the one nobody remembered to enable logging for.
    match open_log(character, payload) {
        Ok(sink) => {
            eprintln!(
                "[log] {}
[log] {}",
                sink.bytes_path().display(),
                sink.events_path().display()
            );
            Session::new(socket).with_sink(sink)
        }
        Err(e) => {
            // A log that cannot be opened must not stop a session. Say so
            // loudly -- silence here reads as "logging worked".
            eprintln!("[log] DISABLED -- could not open a log file: {e}");
            Session::new(socket)
        }
    }
}

/// Open this session's log, with the credentials registered for redaction.
///
/// Returns the sink rather than storing it: the session owns it, one per
/// session, no process-global logger (`plan/05` Rule 5.2).
///
/// **The redaction set is built here, at the one point where every secret is
/// in scope**: the account and character came from the prompt and the key from
/// the `L` response. Registering them anywhere else would mean passing
/// credentials further than they need to go.
fn open_log(character: &str, payload: &cena_platform::LaunchPayload) -> io::Result<SessionSink> {
    let mut redactions = Redactions::new();
    redactions.key(&payload.key);
    // The account name and the account holder's real name arrive in the `A`
    // response, which `authenticate` has already printed by the time this
    // runs. They are registered by the caller of `authenticate` in a later
    // revision; for now the key is the credential that matters most, because
    // it is the one the bytes file would otherwise carry verbatim.
    let dir = cena_platform::log_dir().join(cena_platform::date_dir());
    SessionSink::create(&dir, character, &cena_platform::file_stamp(), redactions)
}

/// Wait for the first room description frame, or time out.
///
/// Criterion 2 is "renders a room description from **typed frames**, never raw
/// text" -- so this matches on a `Frame`, not on the byte stream. That is what
/// "parse first" means (`CLAUDE.md`, settled decisions).
///
/// Returns `false` on timeout or a closed stream; the caller prints the
/// failure, because only it knows what to blame.
async fn wait_for_room(events: &mut broadcast::Receiver<Event>) -> bool {
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
async fn send_manual(handle: &SessionHandle, line: &str) -> cena_session::Outcome {
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
fn print_room(frame: &Frame) {
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

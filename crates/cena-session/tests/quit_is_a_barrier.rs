//! Nothing reaches the wire after `quit`.
//!
//! `SessionHandle::quit`'s doc explains that it rides the command channel
//! "because it must be **ordered behind everything already queued**", so that
//! "the last thing a session did before exiting" is not lost.
//!
//! That is true of the INBOX: arriving at `handle_inbox` means every earlier
//! message has been routed. It is not true of the QUEUE. A command already
//! admitted and waiting for its window is no longer an inbox message; it sits
//! in `CommandQueue` until a prompt closes the window, and `pump` then writes
//! it -- after the `quit` (review SE-2).
//!
//! What that costs:
//!
//! * A write after the server has closed its side tends to produce an RST.
//! * The read arm maps any error to `EndReason::ReadFailed`, which
//!   `warrants_reconnect()` accepts, and the sweep counts the `quit` itself as
//!   attendance -- so a deliberate exit can be followed by a reconnect.
//! * `plan/16` §5b's whole point is distinguishing "the server closed because
//!   we asked" from "the connection dropped". A stray write blurs exactly
//!   that.

use cena_platform::AnsweringSource;
use cena_session::{CommandId, Origin, Session};
use std::time::Duration;

/// The terminator that closes a round-trip window (`plan/12` §4.4).
const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

/// **A queued command must not be written after `quit`.**
///
/// The sequence is the one `handle.rs` describes and does not defend against:
/// a command is admitted and opens a window, the replies are held so the
/// window stays open, `quit` is issued, and then the window closes.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_waiting_on_a_window_is_not_written_after_quit() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    // One full round trip, so the queue is calibrated and idle.
    let first = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(first, cena_session::Outcome::Confirmed(_)),
        "the calibrating round trip must complete: {first:?}"
    );

    // Hold replies, so the NEXT command's window stays open.
    transcript.hold_replies();

    let blocked_handle = handle.clone();
    let blocked = tokio::spawn(async move {
        blocked_handle
            .send_and_await(
                CommandId(2),
                "attack",
                Origin::Manual,
                Duration::from_secs(30),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // A THIRD command, which is admitted to the queue and parked behind the
    // open window. This is the one that must never be written: it has been
    // accepted, but no bytes of it have gone out.
    let queued_handle = handle.clone();
    let queued = tokio::spawn(async move {
        queued_handle
            .send_and_await(
                CommandId(3),
                "stow all",
                Origin::Manual,
                Duration::from_secs(30),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !transcript.lines().iter().any(|l| l == "stow all"),
        "the third command must still be PARKED behind the open window, or \
         this test is not exercising the case: {:?}",
        transcript.lines()
    );

    // Now quit, and then let the window close.
    // Spawned: `quit` waits for the server's EOF, and the point of this test
    // is what happens in the window BEFORE that. Awaiting it here would block
    // until the source hangs up, which is after the moment under test.
    let quit_handle = handle.clone();
    let quitting = tokio::spawn(async move { quit_handle.quit(Duration::from_secs(5)).await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // NOW let the held prompt through, so the open window closes and `pump`
    // gets its chance to write the parked command.
    transcript.release_replies();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let lines = transcript.lines();
    let quit_at = lines.iter().position(|l| l == "quit");
    let stow_at = lines.iter().position(|l| l == "stow all");

    assert!(
        quit_at.is_some(),
        "`quit` must have reached the wire: {lines:?}"
    );
    assert!(
        stow_at.is_none(),
        "`stow all` was written AFTER `quit`. The session asked the server to \
         close and then sent it another command: a write into a closing \
         socket, which tends to produce an RST, which the read arm reports as \
         `ReadFailed`, which warrants a reconnect -- so a deliberate exit can \
         log the character back in. `plan/16` §5b exists to tell 'the server \
         closed because we asked' from 'the connection dropped', and this \
         blurs exactly that. Order was: {lines:?}"
    );

    cancel.cancel();
    let _ = driver.await;
    blocked.abort();
    queued.abort();
    quitting.abort();
}

/// A command issued *after* `quit` is refused rather than written.
///
/// The case above is a command already in the queue. This is the simpler one:
/// nothing should be admitted once the session has asked to leave.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_sent_after_quit_does_not_reach_the_wire() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    tokio::time::sleep(Duration::from_millis(50)).await;
    let farewell = handle.quit(Duration::from_secs(5)).await;

    let after = handle
        .send_and_await(
            CommandId(9),
            "stow all",
            Origin::Manual,
            Duration::from_millis(500),
            cena_session::queue::any_frame,
        )
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let lines = transcript.lines();
    assert!(
        !lines.iter().any(|l| l == "stow all"),
        "a command issued after `quit` reached the wire. The session has \
         already asked the server to close; anything after that is a write \
         into a socket that is going away. Got {after:?}, farewell \
         {farewell:?}, lines {lines:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// **A typed `quit` logs out rather than logging back in.**
///
/// Sent as an ordinary command, `quit` reaches the game, the server closes,
/// and the actor reports `PeerClosed` — which `warrants_reconnect()` accepts,
/// and which the inbox sweep counts as attendance because somebody typed. The
/// supervisor then logs the character straight back in, on every attempt to
/// leave (review SE-3).
///
/// `io.rs` cited Lich's `USER_EXIT_COMMAND` regex only to choose what Cena
/// *sends*; nothing recognised the same intent arriving from a player. The
/// binary has no typed-game-command surface yet, so this was latent — and it
/// goes live the day a frontend adds one, which is when it would be hardest
/// to diagnose.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_typed_quit_is_recognised_as_leaving() {
    for typed in ["quit", "exit", "  QUIT  ", "<c>quit"] {
        let (source, transcript) = AnsweringSource::new(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let cancel = session.cancel_token();
        let driver = tokio::spawn(session.into_actor().run());

        tokio::time::sleep(Duration::from_millis(20)).await;
        let outcome = handle
            .send_and_await(
                CommandId(1),
                typed,
                Origin::Manual,
                Duration::from_secs(5),
                cena_session::queue::any_frame,
            )
            .await;

        // The bytes Cena sends are its own `EXIT_COMMAND`, not the player's
        // spelling: `exit` and `quit` mean the same thing to the game, and
        // the session sends the one it always sends.
        let lines = transcript.lines();
        assert!(
            lines.iter().any(|l| l == "quit"),
            "{typed:?} must put the exit command on the wire: {lines:?}"
        );

        // And it must have gone through the quit machinery, which is what
        // makes the server's close read as "because we asked". A command that
        // merely reached the wire would be answered by the prompt that
        // follows it.
        assert_eq!(
            outcome,
            cena_session::Outcome::Disconnected,
            "{typed:?} must be answered as a session ending, not as an \
             ordinary command awaiting a frame"
        );

        // Nothing further is accepted, which is SE-2's guard reached through
        // this path.
        let after = handle
            .send_and_await(
                CommandId(2),
                "look",
                Origin::Manual,
                Duration::from_millis(200),
                cena_session::queue::any_frame,
            )
            .await;
        assert_eq!(
            after,
            cena_session::Outcome::Disconnected,
            "after a typed {typed:?} the session is leaving and must refuse \
             further commands"
        );

        cancel.cancel();
        let _ = driver.await;
    }
}

/// An ordinary command that merely CONTAINS the word is not an exit.
///
/// Lich's regex anchors, and so must this: `quit guild` and `say quit` are
/// game commands, and treating either as a logout would end the session on a
/// sentence.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_merely_containing_quit_is_not_an_exit() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    tokio::time::sleep(Duration::from_millis(20)).await;
    let outcome = handle
        .send_and_await(
            CommandId(1),
            "say quit already",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;

    let lines = transcript.lines();
    assert!(
        lines.iter().any(|l| l == "say quit already"),
        "an ordinary command must go out verbatim: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l == "quit"),
        "and must NOT be turned into a logout: {lines:?}"
    );
    assert!(
        matches!(outcome, cena_session::Outcome::Confirmed(_)),
        "it is an ordinary round trip: {outcome:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

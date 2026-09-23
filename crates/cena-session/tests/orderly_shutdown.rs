//! **M2 step 7**: `quit`, and waiting for the server to close (`plan/16` §5b).
//!
//! # What the `quit` is actually for
//!
//! > **AUTHOR:** *"i would say yes it should send quit on a clean shutdown. But
//! > the main reason it was done with lich is it sends the signal to lich to
//! > shutdown so it has time to save everything without corruption."*
//!
//! So it is a **shutdown signal to the client layer**, not only a logout to the
//! server -- which is why the ordering in §5b.2 is a contract rather than a
//! nicety, and why the interesting assertion here is not "a `quit` was written"
//! but *what the session does while it waits for the answer*.
//!
//! # The three outcomes, ported from Lich
//!
//! `reference/lich-5/lib/common/orderly_shutdown.rb:181-191` makes three
//! separate checks -- the command sent, the reader stopping in time, and the
//! stream actually reaching EOF -- and each can fail on its own. Cena's
//! [`Farewell`] is those three.
//!
//! **All three still close the socket.** Criterion 6's "no leaked sockets" may
//! not become conditional on the server cooperating, which is the one property
//! in this file that is about safety rather than about politeness.

use cena_platform::AnsweringSource;
use cena_session::{Farewell, Session, State};
use std::time::Duration;

/// A prompt: `plan/12` §4.4's round-trip terminator.
const PROMPT: &[u8] = b"<prompt time=\"1789775900\">&gt;</prompt>\n";

/// Lich's `SERVER_EXIT_TIMEOUT_SECONDS` is 10 (`orderly_shutdown.rb:16`). Tests
/// use it verbatim so a change to the real bound shows up here.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// **The clean exit.** The command goes out, the server closes, and the
/// session reports it was acknowledged.
///
/// The transcript assertion is the load-bearing one: `Farewell::Acknowledged`
/// alone would pass on an implementation that waited for an EOF it never
/// caused.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn quit_is_sent_and_the_servers_close_is_acknowledged() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let actor = session.into_actor();
    let task = tokio::spawn(actor.run());

    // The server closes when asked -- which is what a real one does on `quit`.
    let closing = transcript.clone();
    tokio::spawn(async move {
        while closing.written_count() == 0 {
            tokio::task::yield_now().await;
        }
        closing.hang_up();
    });

    let farewell = handle.quit(EXIT_TIMEOUT).await;
    assert_eq!(
        farewell,
        Farewell::Acknowledged,
        "the server closed the stream, which is Lich's `remote_eof?` \
         (orderly_shutdown.rb:189) -- the exit was registered"
    );

    let lines = transcript.lines();
    assert_eq!(
        lines,
        vec!["quit".to_owned()],
        "and the command REALLY went out. Without this the test would pass on \
         an implementation that waited for an EOF it never asked for."
    );

    let end = task.await.expect("the actor must not panic");
    assert_eq!(end.lifecycle, State::Closed);
    assert!(
        transcript.is_shutdown(),
        "criterion 6: the socket is closed, not leaked"
    );
}

/// **A clean quit must NOT look like a dropped connection.**
///
/// This is the assertion `plan/16` §5b calls out by name:
///
/// > *"A clean `quit` must **not** trigger a reconnect; a drop must."*
///
/// The EOF that ends a quit is byte-for-byte the same `Ok(0)` as a peer hanging
/// up, so the only thing telling them apart is that a quit was pending. Get it
/// wrong and a supervised session logs the character straight back in every
/// time the player tries to leave.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_quit_does_not_warrant_a_reconnect() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let task = tokio::spawn(session.into_actor().run());

    let closing = transcript.clone();
    tokio::spawn(async move {
        while closing.written_count() == 0 {
            tokio::task::yield_now().await;
        }
        closing.hang_up();
    });

    let farewell = handle.quit(EXIT_TIMEOUT).await;
    assert_eq!(farewell, Farewell::Acknowledged);

    let end = task.await.expect("the actor must not panic");
    assert!(
        !end.reason.warrants_reconnect(),
        "a quit ended this session, so it reported {:?} -- and if that \
         warrants a reconnect, every attempt to log out puts the character \
         straight back in-world.",
        end.reason
    );
}

/// **A server that never closes does not hang the session.**
///
/// Lich raises `ServerExitTimeout` here (`orderly_shutdown.rb:188`).
/// `AnsweringSource` never hangs up on its own, so it *is* the unresponsive
/// server -- no extra scaffolding needed.
///
/// The socket still closes. That is the half of this test that is about safety:
/// §5b's ordering note is explicit that `shutdown()` runs even when the `quit`
/// times out.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_server_that_never_closes_times_out_and_the_socket_still_shuts() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let task = tokio::spawn(session.into_actor().run());

    // **Bounded on purpose**, at many times the timeout under test. Falsifying
    // this by disabling the deadline arm did not fail the test -- it HUNG,
    // because a quit with nothing to resolve it waits on a server that never
    // answers. A test whose failure mode is an infinite loop reports nothing to
    // whoever is reading CI, and this is the exact test whose whole subject is
    // "the session does not hang".
    let farewell = tokio::time::timeout(Duration::from_hours(1), handle.quit(EXIT_TIMEOUT))
        .await
        .expect(
            "a quit that is never acknowledged must TIME OUT. Hanging here is \
             the failure this test exists to catch: a player who asked to log \
             out and whose client never stopped.",
        );

    assert_eq!(
        farewell,
        Farewell::TimedOut,
        "the command went out and nothing acknowledged it. Distinct from \
         `Unsent`: the server may yet have registered the logout, which is \
         exactly the ambiguity a log needs to preserve."
    );
    assert_eq!(
        transcript.lines(),
        vec!["quit".to_owned()],
        "it was sent -- `TimedOut` is about the ANSWER, not the asking"
    );

    let end = task.await.expect("the actor must not panic");
    assert!(
        transcript.is_shutdown(),
        "criterion 6 is NOT conditional on the server cooperating \
         (plan/16 §5b, ordering note). A socket left open because the game \
         did not answer is a leaked socket."
    );
    assert!(
        !end.reason.warrants_reconnect(),
        "and a timed-out quit is still a deliberate stop: the player asked to \
         leave, and a rude server does not turn that into a reconnect"
    );
}

/// **Quitting a dead session is not an error.**
///
/// There is nothing to say goodbye to. A caller running an orderly shutdown
/// after the connection already dropped must get an answer rather than hanging
/// on a reply that cannot come.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn quitting_a_dead_session_reports_unsent() {
    let (source, _transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    cancel.cancel();
    let _ = task.await.expect("the actor must not panic");

    // The actor is gone, so the channel is closed.
    let farewell = handle.quit(EXIT_TIMEOUT).await;
    assert_eq!(
        farewell,
        Farewell::Unsent,
        "nothing to send to, and the caller is TOLD rather than left waiting"
    );
}

/// **Commands queued before the quit go out first.**
///
/// This is why the quit rides the command channel rather than arriving out of
/// band: a session's last act must not be lost to a goodbye that overtook it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_quit_is_ordered_behind_commands_already_queued() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let task = tokio::spawn(session.into_actor().run());

    let closing = transcript.clone();
    tokio::spawn(async move {
        // Hang up only once BOTH have been written, or the race decides the
        // assertion instead of the ordering rule doing it.
        while closing.written_count() < 2 {
            // both lines on the wire before the peer goes away
            tokio::task::yield_now().await;
        }
        closing.hang_up();
    });

    // **The `look` must be IN the channel before the quit is offered**, or the
    // test measures task-spawn scheduling rather than channel ordering. The
    // first version spawned the send and awaited the quit immediately, and the
    // quit won -- `send_now` uses `try_send` while `quit` awaits, so the
    // spawned task had not run yet. That failure said nothing about the
    // ordering rule, which is exactly why it had to be staged rather than
    // raced.
    //
    // Holding replies keeps the `look`'s window open, so it is queued and
    // written but unanswered when the quit arrives behind it.
    transcript.hold_replies();
    let sending = handle.clone();
    let sent = tokio::spawn(async move {
        sending
            .send_now(
                "look",
                cena_session::Origin::Manual,
                cena_session::Gate::None,
            )
            .await
    });
    // Yield until the actor has actually written it. Now the ordering under
    // test is the only thing left that can decide the transcript.
    while transcript.written_count() == 0 {
        tokio::task::yield_now().await;
    }
    let farewell = handle.quit(EXIT_TIMEOUT).await;
    let _ = sent.await;

    assert_eq!(
        transcript.lines(),
        vec!["look".to_owned(), "quit".to_owned()],
        "the `look` was queued first and must reach the wire first. A quit \
         delivered out of band could overtake it, and the session's last act \
         would be lost."
    );
    assert_eq!(farewell, Farewell::Acknowledged);

    let _ = task.await;
}

/// A server that answers `quit` by RESETTING the connection rather than
/// closing it: one prompt, then a read error once `quit` has been written.
struct ResetOnQuit {
    prompted: bool,
    quit: bool,
}

impl cena_platform::ByteSource for ResetOnQuit {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.prompted {
            self.prompted = true;
            buf[..PROMPT.len()].copy_from_slice(PROMPT);
            return Ok(PROMPT.len());
        }
        if self.quit {
            return Err(std::io::ErrorKind::ConnectionReset.into());
        }
        std::future::pending().await
    }

    async fn write_all(&mut self, message: &[u8]) -> std::io::Result<()> {
        self.quit |= message == b"quit\n";
        Ok(())
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// **A reset after a quit is still the goodbye.**
///
/// Only the EOF arm asked whether a quit was pending, so a quit answered by a
/// reset -- or a link that dropped in the same moment -- ended as
/// `ReadFailed`, and the supervisor logged the character straight back in.
/// VERIFIED before the fix: a typed `quit` then a reset gave two connect
/// attempts and generation 1.
///
/// Both ways in are covered, because they are two code paths: the handle's
/// `quit`, and a player TYPING it -- which `cena-web` sends since M4.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_quit_answered_by_a_reset_does_not_warrant_a_reconnect() {
    for typed in [false, true] {
        let session = Session::new(ResetOnQuit {
            prompted: false,
            quit: false,
        });
        let handle = session.handle();
        let task = tokio::spawn(session.into_actor().run());

        if typed {
            let _ = handle
                .send_manual_at(handle.generation(), "quit", EXIT_TIMEOUT)
                .await;
        } else {
            let _ = handle.quit(EXIT_TIMEOUT).await;
        }

        let end = task.await.expect("the actor must not panic");
        assert!(
            !end.reason.warrants_reconnect(),
            "typed={typed}: a quit answered by a reset ended as {:?}, which \
             reconnects -- the player asked to leave and was put back in-world",
            end.reason
        );
    }
}

/// A socket whose send buffer never drains: every write parks forever.
struct StalledWrite;

impl cena_platform::ByteSource for StalledWrite {
    async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        std::future::pending().await
    }

    async fn write_all(&mut self, _message: &[u8]) -> std::io::Result<()> {
        std::future::pending().await
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// **A cancel interrupts a write in progress, and is reported as a cancel.**
///
/// Every write is awaited outside the loop's `select!`, so the cancel arm
/// could not fire until the write gave up: VERIFIED before the fix, a stop
/// took the full `WRITE_DEADLINE` (5 s) against `plan/12` §4.3's 250 ms
/// `PREEMPT_GRACE`, and the session ended `WriteFailed` -- a lost transport,
/// which a supervisor answers `Disconnected` and reconnects (review
/// finding 5).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_cancel_interrupts_a_stalled_write() {
    const PREEMPT_GRACE: Duration = Duration::from_millis(250);
    let session = Session::new(StalledWrite);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());
    let waiting = tokio::spawn(async move {
        handle
            .send_and_await(
                cena_session::CommandId(1),
                "look",
                cena_session::Origin::Manual,
                Duration::from_mins(1),
                cena_session::queue::any_frame,
            )
            .await
    });
    // The command is now mid-write, and will stay there.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let start = tokio::time::Instant::now();
    cancel.cancel();
    let end = task.await.expect("the actor must not panic");
    let took = start.elapsed();

    assert!(
        took <= PREEMPT_GRACE,
        "the stop took {took:?}: it waited for the stalled write to time out"
    );
    assert_eq!(
        end.reason,
        cena_session::EndReason::Cancelled,
        "a stop is a cancel, not a lost transport a supervisor would reconnect"
    );
    assert_eq!(
        waiting.await.expect("the waiter must not panic"),
        cena_session::Outcome::Dead,
        "the command caught in the write is answered, not dropped"
    );
}

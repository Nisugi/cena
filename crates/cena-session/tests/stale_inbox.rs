//! **SE-1: a message parked between generations runs on the NEXT connection.**
//!
//! The command receiver is deliberately durable across generations
//! (`supervisor/core.rs:41-43`: *"a handle taken in generation 0 still reaches
//! the actor in generation 3"*). That is right for handles, and it is exactly
//! what lets a message outlive the connection it was meant for.
//!
//! Between `actor.run()` returning and the next one starting, the receiver sits
//! **un-polled** while handles still `try_send` successfully. The supervisor
//! then hands the same receiver to the new actor, which drains it.
//!
//! The worst case is `quit`. `handle.rs:566` bounds the caller's wait, so a
//! `quit` issued while the session is reconnecting comes back `Unsent` -- which
//! its own comment defines as *"assume the game was not told"*. But the
//! `Inbox::Quit` is still in the channel, and `io.rs` acts on it
//! unconditionally, so the next successful login is followed immediately by a
//! logout the caller was told had not been sent.
//!
//! §5.1 says commands during `Reconnecting` "fail immediately with
//! `Disconnected`".

use cena_platform::ReplaySource;
use cena_session::{
    CommandId, ConnectError, Connector, Farewell, Generation, Origin, Outcome, SupervisedSession,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// A connector that fails once, then succeeds -- forcing exactly one reconnect.
///
/// The failure is **transient**, because a fatal one stops the ladder and there
/// would be no second connection for a stale message to land on.
struct FlakyConnector {
    sources: VecDeque<Vec<Vec<u8>>>,
    fail_on: u32,
    attempts: Arc<AtomicU32>,
}

impl Connector for FlakyConnector {
    type Source = ReplaySource;

    async fn connect(&mut self, _generation: Generation) -> Result<ReplaySource, ConnectError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == self.fail_on {
            return Err(ConnectError::transient("scripted", "a flaky connect"));
        }
        self.sources
            .pop_front()
            .map(ReplaySource::new)
            .ok_or_else(|| ConnectError::fatal("scripted", "no more prepared connections"))
    }
}

/// A connection that teaches a prompt and then keeps the stream open.
fn a_live_connection() -> Vec<Vec<u8>> {
    vec![b"<prompt time=\"1789775821\">R&gt;</prompt>\n".to_vec()]
}

#[tokio::test(start_paused = true)]
async fn a_quit_answered_unsent_does_not_log_out_the_next_connection() {
    let attempts = Arc::new(AtomicU32::new(0));
    let connector = FlakyConnector {
        sources: vec![a_live_connection(), a_live_connection()].into(),
        // Fail the SECOND connect, so generation 0 succeeds, dies, and the
        // retry ladder runs before generation 1.
        fail_on: 1,
        attempts: Arc::clone(&attempts),
    };

    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());

    // Let generation 0 come up and then end (the replay runs out -> Ok(0)).
    tokio::time::sleep(Duration::from_millis(50)).await;

    // A `quit` issued while the session is between connections. The caller is
    // told `Unsent`, whose documented meaning is "assume the game was not
    // told".
    let farewell = handle.quit(Duration::from_millis(10)).await;
    assert_eq!(
        farewell,
        Farewell::Unsent,
        "a quit during a reconnect must answer Unsent -- the game was not told"
    );

    // Now let the ladder run and generation 1 come up.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // **THE DEFECT, asserted on the WIRE.** The recorder is the evidence: if
    // the parked `Inbox::Quit` survived, a `quit` line appears as an outbound
    // event
    // on the second connection -- a logout the caller was explicitly told had
    // not been sent.
    //
    // Asserted on outbound bytes rather than on "is the session still running",
    // which was the first cut and was WRONG: both scripted replays run out on
    // their own, so the session stops either way and the assertion passed and
    // failed for reasons unrelated to the defect.
    cancel.cancel();
    let mut end = tokio::time::timeout(Duration::from_secs(2), running)
        .await
        .expect("the supervisor must stop")
        .expect("and not panic");

    let outbound: Vec<String> = end
        .recorder
        .events()
        .iter()
        .filter_map(|event| match event {
            cena_platform::RecordedEvent::Outbound { bytes, .. } => {
                Some(String::from_utf8_lossy(bytes).into_owned())
            }
            cena_platform::RecordedEvent::Inbound { .. } => None,
        })
        .collect();

    assert!(
        !outbound.iter().any(|line| line.trim() == "quit"),
        "`quit` reached the wire after the caller was told `Unsent`.          `Farewell::Unsent` means 'assume the game was not told'; sending it          later makes that answer a lie and logs the character out on a          connection the caller never asked to end. Outbound was: {outbound:?}"
    );

    // The session DID reconnect -- otherwise there was no second connection for
    // a stale message to land on, and this test would pass vacuously.
    // At LEAST a second generation: the sweep counts a discarded command as
    // attendance (somebody typed), so the unattended cap lets the ladder run
    // further than the bare minimum. The number is not the point -- reaching a
    // second connection at all is, because without one this test would pass
    // vacuously with nothing for a stale message to land on.
    assert!(
        end.generations > Generation(0),
        "the test must actually reach a second generation, or there was no          second connection for a stale `quit` to be replayed against"
    );
}

/// A command parked during a reconnect answers `Disconnected`, not `Dead`.
///
/// `plan/12` §5.1 says commands during `Reconnecting` *"fail immediately with
/// `Disconnected`"*, and the distinction is load-bearing: `Dead` means the
/// session is gone for good, `Disconnected` means this connection died and a
/// retry may work. A behavior that cannot tell them apart cannot decide between
/// giving up and waiting.
#[tokio::test(start_paused = true)]
async fn a_command_parked_during_a_reconnect_answers_disconnected() {
    let attempts = Arc::new(AtomicU32::new(0));
    let connector = FlakyConnector {
        sources: vec![a_live_connection(), a_live_connection()].into(),
        fail_on: 1,
        attempts: Arc::clone(&attempts),
    };

    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Sent while the session is between connections, so it parks in the
    // durable channel rather than reaching an actor. Spawned because
    // `send_and_await` does not return until it is answered -- which is the
    // point: the drain is what answers it.
    let queued = tokio::spawn(async move {
        handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Manual,
                Duration::from_secs(30),
                cena_session::queue::any_frame,
            )
            .await
    });

    tokio::time::sleep(Duration::from_secs(5)).await;
    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), running).await;

    let outcome = tokio::time::timeout(Duration::from_secs(1), queued)
        .await
        .expect("the waiter must be answered, not left hanging")
        .expect("and the task must not panic");

    assert_eq!(
        outcome,
        Outcome::Disconnected,
        "a command parked across a reconnect must answer `Disconnected` -- \
         `Dead` would tell a behavior to give up on a session that is alive"
    );
}

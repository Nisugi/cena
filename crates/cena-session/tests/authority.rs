//! The command authority belongs to the session (SE-4, `plan/30` §6 Q3), and
//! an explicit stop can take it back (`plan/12` §4.3).

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::queue::any_frame;
use cena_session::{
    AuthorityToken, CommandId, ConnectError, Connector, Event, Generation, Origin, Outcome,
    PREEMPT_GRACE, Preempted, Refusal, Session, State, SupervisedSession,
};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

/// Every connect logs in; the test hangs one up when it chooses.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<TranscriptHandle>>>);

impl Lines {
    fn latest(&self) -> Option<TranscriptHandle> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .cloned()
    }
}

struct HangUps(Lines);

impl Connector for HangUps {
    type Source = AnsweringSource;

    async fn connect(&mut self, _generation: Generation) -> Result<AnsweringSource, ConnectError> {
        let (source, transcript) = AnsweringSource::logged_in(PROMPT);
        self.0
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(transcript);
        Ok(source)
    }
}

/// Wait for the next `Ready`, bounded so a stopped session fails rather
/// than hangs.
async fn ready(events: &mut tokio::sync::broadcast::Receiver<Event>) -> bool {
    let next = async {
        loop {
            match events.recv().await {
                Ok(Event::StateChanged(State::Ready)) => return true,
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return false,
            }
        }
    };
    tokio::time::timeout(Duration::from_hours(1), next)
        .await
        .unwrap_or(false)
}

/// SE-4: a behavior holding the authority keeps it through a reconnect, and
/// its commands on the new connection are sent -- not refused `Permanent`,
/// "will never succeed", on a session that is alive (`plan/19`).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_holder_keeps_the_authority_across_a_reconnect() {
    let lines = Lines::default();
    let (session, handle) = SupervisedSession::new(HangUps(lines.clone()));
    let (_, mut events) = session.subscribe();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());
    let hunt = AuthorityToken(7);

    assert!(ready(&mut events).await);
    handle.claim(hunt).await.expect("nobody holds it yet");
    lines.latest().expect("a connection").hang_up();
    assert!(ready(&mut events).await, "the session reconnects");

    assert_eq!(handle.holder(), Some(hunt));
    let outcome = handle
        .send_and_await(
            CommandId(1),
            "attack",
            Origin::Behavior(hunt),
            DEADLINE,
            any_frame,
        )
        .await;
    // A prompt alone matches nothing, so the answer is `Timeout`: sent and
    // answered, "no match within the window" (`plan/12` §4.4). What matters
    // is that it was not refused, and reached the new connection.
    assert_eq!(outcome, Outcome::Timeout);
    assert_eq!(lines.latest().expect("a connection").lines(), ["attack"]);
    assert!(
        handle.claim(AuthorityToken(8)).await.is_err(),
        "nobody else could take it while the connection was down"
    );
    cancel.cancel();
    let _ = task.await;
}

/// A release between connections holds, too: the next behavior may claim.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_release_during_a_reconnect_frees_the_authority() {
    let lines = Lines::default();
    let (session, handle) = SupervisedSession::new(HangUps(lines.clone()));
    let (_, mut events) = session.subscribe();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());

    assert!(ready(&mut events).await);
    handle.claim(AuthorityToken(7)).await.expect("free");
    lines.latest().expect("a connection").hang_up();
    handle.release(AuthorityToken(7));
    assert!(ready(&mut events).await);

    assert_eq!(handle.claim(AuthorityToken(8)).await, Ok(()));
    cancel.cancel();
    let _ = task.await;
}

/// A session with a holder, one command of its in flight and more queued.
async fn a_busy_holder() -> (
    cena_session::SessionHandle,
    TranscriptHandle,
    CancellationToken,
    Vec<tokio::task::JoinHandle<Outcome>>,
) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, mut events) = session.subscribe();
    let handle = session.handle();
    let cancel = session.cancel_token();
    tokio::spawn(session.into_actor().run());
    assert!(ready(&mut events).await);
    assert_eq!(handle.claim(AuthorityToken(7)).await, Ok(()));
    // The first command's answer is held, so the others queue behind it.
    transcript.hold_replies();
    let mut sends = Vec::new();
    for n in 1..=3 {
        let handle = handle.clone();
        sends.push(tokio::spawn(async move {
            handle
                .send_and_await(
                    CommandId(n),
                    "attack",
                    Origin::Behavior(AuthorityToken(7)),
                    Duration::from_mins(1),
                    any_frame,
                )
                .await
        }));
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(1)).await;
    (handle, transcript, cancel, sends)
}

/// §4.3: a holder that ignores its stop loses the authority after
/// `PREEMPT_GRACE`, and its queued commands are refused rather than sent --
/// an attack from a stopped behavior is what a stop exists to prevent.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_holder_that_ignores_its_stop_is_revoked() {
    let (handle, transcript, cancel, sends) = a_busy_holder().await;
    // Nothing listens to this token: the "behavior" ignores it.
    let stop = CancellationToken::new();

    let started = tokio::time::Instant::now();
    assert_eq!(
        handle.preempt(&stop).await,
        Preempted::Revoked(AuthorityToken(7))
    );
    assert!(started.elapsed() <= PREEMPT_GRACE + Duration::from_millis(20));
    assert!(stop.is_cancelled(), "the holder was told first");
    assert_eq!(handle.holder(), None);

    transcript.release_replies();
    let mut outcomes = Vec::new();
    for send in sends {
        outcomes.push(send.await.expect("no panic"));
    }
    // Answered by its prompt; a prompt matches nothing, hence `Timeout`.
    assert_eq!(outcomes[0], Outcome::Timeout, "{outcomes:?}");
    assert_eq!(
        &outcomes[1..],
        [
            Outcome::Refused(Refusal::Permanent),
            Outcome::Refused(Refusal::Permanent)
        ]
    );
    assert_eq!(
        transcript.lines(),
        ["attack"],
        "only the command already on the wire"
    );
    assert_eq!(handle.claim(AuthorityToken(8)).await, Ok(()));
    cancel.cancel();
}

/// A holder that lets go when told is not revoked.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_holder_that_lets_go_yields() {
    let (source, _transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, mut events) = session.subscribe();
    let handle = session.handle();
    let cancel = session.cancel_token();
    tokio::spawn(session.into_actor().run());
    assert!(ready(&mut events).await);
    handle.claim(AuthorityToken(7)).await.expect("free");

    let stop = CancellationToken::new();
    let behaving = {
        let (handle, stop) = (handle.clone(), stop.clone());
        tokio::spawn(async move {
            stop.cancelled().await;
            handle.release(AuthorityToken(7));
        })
    };
    assert_eq!(
        handle.preempt(&stop).await,
        Preempted::Yielded(AuthorityToken(7))
    );
    behaving.await.expect("no panic");
    cancel.cancel();
}

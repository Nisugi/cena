//! A wedged behavior is stopped (`plan/12` §5.5); a slow one is not.
//!
//! "Wedged" is a loop that stopped turning, not a behavior that stopped
//! sending: a hunt resting in town sends nothing for minutes and is fine.

mod ready;

use cena_behavior::watchdog::{Heartbeat, Watched, watch};
use cena_platform::AnsweringSource;
use cena_session::{AuthorityToken, Event, Preempted, Session, SessionHandle};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const LIMIT: Duration = Duration::from_secs(30);
const TOKEN: AuthorityToken = AuthorityToken(5);

async fn a_ready_session() -> (SessionHandle, tokio::sync::broadcast::Receiver<Event>) {
    let (source, _transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, ready) = session.subscribe();
    let (_, events) = session.subscribe();
    let handle = session.handle();
    tokio::spawn(session.into_actor().run());
    assert!(
        ready::until_ready(ready).await.is_ok(),
        "the session becomes Ready"
    );
    (handle, events)
}

/// A behavior whose loop stops turning -- stuck in an await that ignores its
/// stop -- is preempted: its authority is taken, and the player is told.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_whose_loop_stops_is_taken_off_the_character() {
    let (handle, mut events) = a_ready_session().await;
    handle.claim(TOKEN).await.expect("free");
    let (stop, heartbeat) = (CancellationToken::new(), Heartbeat::default());
    // Turns its loop for a minute, then wedges without releasing.
    let behavior = {
        let heartbeat = heartbeat.clone();
        tokio::spawn(async move {
            for _ in 0..240 {
                heartbeat.beat();
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            std::future::pending::<()>().await;
        })
    };

    let started = tokio::time::Instant::now();
    // Bounded, so a watchdog that never fires fails here rather than hangs.
    let watched = tokio::time::timeout(
        Duration::from_mins(10),
        watch(&handle, &stop, &heartbeat, LIMIT, "Hunt"),
    )
    .await;

    assert_eq!(watched, Ok(Watched::Wedged(Preempted::Revoked(TOKEN))));
    let took = started.elapsed();
    assert!(
        took >= Duration::from_mins(1) + LIMIT && took <= Duration::from_mins(1) + LIMIT * 2,
        "stopped {took:?} after starting: not while it was turning, and not long after"
    );
    assert_eq!(handle.holder(), None);
    let mut told = false;
    while let Ok(event) = events.try_recv() {
        if let Event::Notice(notice) = event {
            told |= notice
                .lines()
                .iter()
                .any(|l| l.contains("Hunt stopped responding"));
        }
    }
    assert!(told, "the player is told what was stopped");
    behavior.abort();
}

/// A behavior that sends nothing for minutes but keeps turning its loop --
/// resting, waiting out a ferry -- is left alone.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_that_waits_but_turns_is_left_alone() {
    let (handle, _events) = a_ready_session().await;
    handle.claim(TOKEN).await.expect("free");
    let (stop, heartbeat) = (CancellationToken::new(), Heartbeat::default());
    let behavior = {
        let (heartbeat, stop) = (heartbeat.clone(), stop.clone());
        tokio::spawn(async move {
            // Ten minutes of waiting, with no command sent.
            for _ in 0..2_400 {
                heartbeat.beat();
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            stop.cancel();
        })
    };

    let watched = watch(&handle, &stop, &heartbeat, LIMIT, "Hunt").await;

    assert_eq!(watched, Watched::Stopped);
    assert_eq!(handle.holder(), Some(TOKEN), "nothing was taken");
    behavior.await.expect("no panic");
}

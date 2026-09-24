//! "Is anyone here?" is asked of a **person**, not of the traffic
//! (`plan/30` §6 Q5, `command/attendance.rs`).
//!
//! The author, 2026-09-24, of logging a character in elsewhere while Hydra
//! hunts: *"yes it should give him up"*. So a behavior sending all the time
//! must not make a session look attended, or Hydra and the phone fight over
//! the character forever. And a hunt left alone overnight must survive the
//! occasional network blip, which a long-lived connection tells apart.

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    ConnectError, Connector, Event, Gate, Generation, LONG_LIVED, Origin, State, StoppedBecause,
    SupervisedSession,
};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

/// Every connect logs in; the test hangs each one up when it chooses, the
/// way another client logging the character in would.
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
    fn made(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len()
    }
    fn written(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .flat_map(TranscriptHandle::lines)
            .collect()
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

/// Wait for the next `Ready`: `false` if none comes. **Bounded**, so a
/// session that stopped ends the wait rather than hanging the test -- a live
/// handle keeps the event channel open, so its closing cannot be what ends it.
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

/// A behavior sending on every connection, with nobody there: each login is
/// knocked off seconds later, as by a phone. Hydra gives the character up.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_sending_is_not_a_person_so_a_fight_is_given_up() {
    let lines = Lines::default();
    let (session, handle) = SupervisedSession::new(HangUps(lines.clone()));
    let (_, mut events) = session.subscribe();
    let task = tokio::spawn(session.run());

    for _ in 0..5 {
        if !ready(&mut events).await {
            break;
        }
        let _ = handle
            .send_now(
                "attack",
                Origin::Behavior(cena_session::AuthorityToken(1)),
                Gate::None,
            )
            .await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        lines.latest().expect("a connection").hang_up();
    }
    let end = tokio::time::timeout(Duration::from_hours(1), task)
        .await
        .expect("the session must stop, not fight on")
        .expect("no panic");

    assert!(
        lines.written().iter().any(|line| line == "attack"),
        "the behavior's command must have reached the wire, or this proves nothing"
    );
    assert_eq!(end.stopped_because, StoppedBecause::Unattended);
    assert_eq!(
        lines.made(),
        2,
        "given up after two losses with nobody there"
    );
}

/// The same fight with a person typing keeps going: someone is there.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_person_typing_keeps_the_session() {
    let lines = Lines::default();
    let (session, handle) = SupervisedSession::new(HangUps(lines.clone()));
    let (_, mut events) = session.subscribe();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());

    for _ in 0..4 {
        assert!(ready(&mut events).await, "a person was there");
        let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        lines.latest().expect("a connection").hang_up();
    }
    assert!(ready(&mut events).await);
    assert!(
        !task.is_finished(),
        "a person was there on every connection"
    );
    cancel.cancel();
    let _ = task.await;
    assert_eq!(lines.made(), 5);
}

/// A hunt left overnight: nobody types, but each connection lived for hours
/// before a network blip. Blips are not a fight, and the hunt survives them.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_long_lived_connection_counts_as_attended() {
    let lines = Lines::default();
    let (session, _handle) = SupervisedSession::new(HangUps(lines.clone()));
    let (_, mut events) = session.subscribe();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());

    for _ in 0..3 {
        assert!(ready(&mut events).await, "a blip is not a fight");
        tokio::time::sleep(LONG_LIVED + Duration::from_secs(1)).await;
        lines.latest().expect("a connection").hang_up();
    }
    assert!(ready(&mut events).await);
    assert!(
        !task.is_finished(),
        "three blips, hours apart, stopped the hunt"
    );
    cancel.cancel();
    let _ = task.await;
    assert_eq!(lines.made(), 4);
}

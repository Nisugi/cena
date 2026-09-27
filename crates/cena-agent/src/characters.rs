//! The characters an agent can see: the ones this Hydra is running, each with
//! its session's read-only observer and a log of what happened to it.
//!
//! The binary seats a character when it starts one and unseats it when it
//! stops, as it attaches and detaches Despana's pages (`crates/cena/src/play.rs`).
//! Seating starts a watcher that fills the character's [`Log`]; unseating
//! stops it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cena_session::{Event, ObserveError, ObservedEvent, SessionId, SessionObserver, Snapshot};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::happenings::{Happening, Log, changed, diff};
use crate::projection::{CharacterState, project};

/// A seated character, as the tools read it.
#[derive(Clone, Debug)]
pub struct Seat {
    /// The name Hydra runs it as.
    pub name: String,
    /// Read-only access to its session.
    pub observer: SessionObserver,
    /// Its database (the combat recorder and the loot ledger), when known.
    pub database: Option<PathBuf>,
    /// Whether this run records combat and loot into it (`--record`);
    /// `None` when whoever seated it did not say.
    pub recording: Option<bool>,
    /// What happened to it.
    pub log: Arc<Log>,
    stop: CancellationToken,
}

/// Every seated character. Cloned freely: one table behind it.
#[derive(Clone, Debug, Default)]
pub struct Characters {
    seats: Arc<Mutex<BTreeMap<SessionId, Seat>>>,
}

impl Characters {
    /// Seat a character, and start watching it. Called inside a Tokio
    /// runtime.
    pub fn seat(
        &self,
        id: SessionId,
        name: &str,
        observer: SessionObserver,
        database: Option<PathBuf>,
        recording: Option<bool>,
    ) {
        let seat = Seat {
            name: name.to_owned(),
            observer,
            database,
            recording,
            log: Arc::default(),
            stop: CancellationToken::new(),
        };
        tokio::spawn(watch(
            seat.name.clone(),
            seat.observer.clone(),
            Arc::clone(&seat.log),
            seat.stop.clone(),
        ));
        if let Some(old) = self.lock().insert(id, seat) {
            old.stop.cancel();
        }
    }

    /// Unseat a character, and stop watching it.
    pub fn unseat(&self, id: SessionId) {
        if let Some(seat) = self.lock().remove(&id) {
            seat.stop.cancel();
            seat.log.close();
        }
    }

    /// Every seated character, in the order they were seated.
    #[must_use]
    pub fn all(&self) -> Vec<Seat> {
        self.lock().values().cloned().collect()
    }

    /// A seated character by name, ignoring case.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<Seat> {
        self.lock()
            .values()
            .find(|seat| seat.name.eq_ignore_ascii_case(name.trim()))
            .cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, Seat>> {
        self.seats.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A fresh snapshot and the events after it, retrying the failures that are
/// not an answer (`Busy`, `Timeout`) with a bounded backoff, as Despana's pump
/// does. `None` when stopped or the session is gone.
pub async fn subscribe(
    observer: &SessionObserver,
    stop: &CancellationToken,
) -> Option<(Snapshot, broadcast::Receiver<ObservedEvent>)> {
    let mut backoff = Duration::from_millis(50);
    loop {
        match observer.subscribe().await {
            Ok(subscription) => return Some(subscription),
            Err(ObserveError::Busy | ObserveError::Timeout) => {
                tokio::select! {
                    () = stop.cancelled() => return None,
                    () = tokio::time::sleep(backoff) => {}
                }
                backoff = (backoff * 2).min(Duration::from_secs(2));
            }
            Err(_) => return None,
        }
    }
}

/// The session's own events that are happenings as they are.
fn direct(event: &Event) -> Option<Happening> {
    match event {
        Event::Sent { line, origin } => Some(Happening::Sent {
            line: line.clone(),
            origin: format!("{origin:?}").to_ascii_lowercase(),
        }),
        Event::Notice(notice) => Some(Happening::Notice {
            text: notice.lines().join("\n"),
        }),
        Event::StateChanged(state) => Some(Happening::Lifecycle {
            state: format!("{state:?}").to_ascii_lowercase(),
        }),
        _ => None,
    }
}

/// Whether the state may have changed enough to look again: a prompt closed
/// a chunk, combat was folded, or the lifecycle moved.
fn worth_a_look(event: &Event) -> bool {
    match event {
        Event::Frame(frame) => matches!(**frame, cena_session::Frame::Prompt { .. }),
        Event::Combat(_) | Event::StateChanged(_) => true,
        _ => false,
    }
}

/// Watch one character until stopped: its direct events into the log as
/// they come, and after each prompt a fresh snapshot, compared with the last.
async fn watch(name: String, observer: SessionObserver, log: Arc<Log>, stop: CancellationToken) {
    let Some((snapshot, mut events)) = subscribe(&observer, &stop).await else {
        log.close();
        return;
    };
    let mut last: CharacterState = project(&name, &snapshot);
    log.reached(snapshot.cursor);
    loop {
        let received = tokio::select! {
            () = stop.cancelled() => return,
            received = events.recv() => received,
        };
        let mut gap = false;
        let look = match received {
            Ok(event) => {
                if let Some(happening) = direct(&event.event) {
                    log.push(event.cursor, happening);
                }
                log.reached(event.cursor);
                worth_a_look(&event.event)
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                gap = true;
                true
            }
            Err(broadcast::error::RecvError::Closed) => {
                log.close();
                return;
            }
        };
        if !look {
            continue;
        }
        let Some((snapshot, fresh)) = subscribe(&observer, &stop).await else {
            log.close();
            return;
        };
        // The events up to the new snapshot's fence are still in the old
        // receiver: take their direct happenings before letting it go.
        while let Ok(event) = events.try_recv() {
            if event.cursor > snapshot.cursor {
                break;
            }
            if let Some(happening) = direct(&event.event) {
                log.push(event.cursor, happening);
            }
        }
        events = fresh;
        if gap {
            log.push(snapshot.cursor, Happening::Gap);
        }
        let now = project(&name, &snapshot);
        for happening in diff(&last, &now).into_iter().chain(changed(&last, &now)) {
            log.push(snapshot.cursor, happening);
        }
        log.reached(snapshot.cursor);
        last = now;
    }
}

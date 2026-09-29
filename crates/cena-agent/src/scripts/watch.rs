//! What a runner is told, and when (`plan/46` §3, §4.1): the session's
//! events a script reads, and its local copy's changes, in an order that
//! keeps Lich's promise.
//!
//! **State first, then the line** (`inventory/13` §1.1: Lich updates its
//! state before a script sees the line). The model changes as each frame
//! arrives, but a copy of it is only taken whole at a prompt, where a chunk
//! of the game's text ends. So the lines of a chunk are **held until its
//! prompt**: then the copy's changes go first, then the held lines, then the
//! prompt. A script woken by a line reads the state that line produced, or
//! later -- as under Lich, where the parser runs ahead of a script's thread.
//! A chunk with no prompt is let go after [`HOLD`], without a copy.
//!
//! **A built-in's end is told the same way**: a copy is taken first, so a
//! script that waited for a walk and then reads `Room.current` reads the
//! room it arrived in, not the one it left (`plan/46` §7).
//!
//! A copy is taken as the agent's watcher takes one (`crate::characters`):
//! a fresh snapshot and its stream at each prompt, the old stream read up
//! to the new snapshot's fence first, so nothing between falls out.

use std::sync::Arc;
use std::time::Duration;

use cena_session::{Event, Frame, ObservedEvent, SessionObserver, Snapshot};
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::listening::{self, Listening};
use super::local::{Atlas, Local, Whereabouts, local};
use crate::happenings::changed_fields;

/// How long a chunk's lines wait for their prompt before they go without
/// it. The game ends almost every chunk with one, in the same read.
pub const HOLD: Duration = Duration::from_millis(250);

/// One runner's watch.
pub(super) struct Watching {
    pub(super) character: String,
    pub(super) observer: SessionObserver,
    pub(super) atlas: Option<Arc<Atlas>>,
    pub(super) listening: Arc<Listening>,
    pub(super) stop: CancellationToken,
    /// The runner's built-ins that ended: told after a fresh copy.
    pub(super) ends: mpsc::UnboundedReceiver<listening::Event>,
}

/// What a runner is told of one of the session's events, if anything.
fn told(observed: &ObservedEvent) -> Option<listening::Event> {
    let cursor = observed.cursor;
    match &observed.event {
        Event::Heard(line) => Some(listening::Event::Line {
            cursor,
            stream: line.stream.clone(),
            text: line.text(),
        }),
        Event::Sent { line, origin, .. } => Some(listening::Event::Sent {
            cursor,
            line: line.clone(),
            origin: origin.word().to_owned(),
        }),
        Event::Frame(frame) => match frame.as_ref() {
            Frame::Prompt { time, text } => Some(listening::Event::Prompt {
                cursor,
                time: time.parse().ok(),
                text: text.clone(),
            }),
            _ => None,
        },
        Event::StateChanged(state) => Some(listening::Event::Lifecycle {
            cursor,
            state: format!("{state:?}").to_ascii_lowercase(),
            generation: u64::from(observed.generation.0),
        }),
        _ => None,
    }
}

/// Whether a chunk ended, so a copy is due: a prompt, or the lifecycle
/// moving (a reconnect's state is new).
fn closes_a_chunk(event: &Event) -> bool {
    match event {
        Event::Frame(frame) => matches!(**frame, Frame::Prompt { .. }),
        Event::StateChanged(_) => true,
        _ => false,
    }
}

/// The watch under way.
struct Watcher {
    character: String,
    observer: SessionObserver,
    atlas: Option<Arc<Atlas>>,
    listening: Arc<Listening>,
    stop: CancellationToken,
    events: broadcast::Receiver<ObservedEvent>,
    whereabouts: Whereabouts,
    last: Local,
    held: Vec<listening::Event>,
    let_go: Option<Instant>,
}

impl Watcher {
    fn copy_of(&mut self, snapshot: &Snapshot) -> Local {
        let map_room = self
            .whereabouts
            .locate(self.atlas.as_deref(), &snapshot.state);
        local(&self.character, snapshot, map_room)
    }

    /// Take a fresh copy and tell what changed, then what was held. `false`
    /// when the session is gone.
    async fn copy(&mut self) -> bool {
        let Some((snapshot, fresh)) =
            crate::characters::subscribe(&self.observer, &self.stop).await
        else {
            return false;
        };
        // The events up to the new snapshot's fence are still in the old
        // receiver: they belong to this chunk. Some lost on the way are
        // said to have been, as the watch says it (the integrated crate
        // review of 2026-09-28, I5: this stopped at a loss, silently).
        loop {
            match self.events.try_recv() {
                Ok(event) if event.cursor <= snapshot.cursor => self.held.extend(told(&event)),
                Err(broadcast::error::TryRecvError::Lagged(missed)) => {
                    self.held.push(listening::Event::Lagged { missed });
                }
                Ok(_) | Err(_) => break,
            }
        }
        self.events = fresh;
        let now = self.copy_of(&snapshot);
        let fields = changed_fields(Some(&self.last), &now);
        if !fields.is_empty() {
            self.listening.push(listening::Event::State {
                cursor: snapshot.cursor,
                fields,
            });
        }
        self.last = now;
        self.release();
        true
    }

    /// Pass on what was held, in order.
    fn release(&mut self) {
        for event in self.held.drain(..) {
            self.listening.push(event);
        }
        self.let_go = None;
    }

    fn end(&mut self) {
        self.release();
        self.listening.close();
    }
}

/// Keep what the session publishes for one runner, starting from
/// `snapshot` and its `events`, until stopped.
pub(super) async fn watch(
    watching: Watching,
    snapshot: Snapshot,
    events: broadcast::Receiver<ObservedEvent>,
) {
    let Watching {
        character,
        observer,
        atlas,
        listening,
        stop,
        mut ends,
    } = watching;
    let mut whereabouts = Whereabouts::default();
    let map_room = whereabouts.locate(atlas.as_deref(), &snapshot.state);
    let mut watcher = Watcher {
        last: local(&character, &snapshot, map_room),
        character,
        observer,
        atlas,
        listening,
        stop,
        events,
        whereabouts,
        held: Vec::new(),
        let_go: None,
    };
    watcher.listening.push(listening::Event::State {
        cursor: snapshot.cursor,
        fields: changed_fields(None, &watcher.last),
    });
    loop {
        let received = tokio::select! {
            () = watcher.stop.cancelled() => return,
            () = sleep_until(watcher.let_go) => {
                watcher.release();
                continue;
            }
            Some(ended) = ends.recv() => {
                if !watcher.copy().await {
                    watcher.end();
                    return;
                }
                watcher.listening.push(ended);
                continue;
            }
            received = watcher.events.recv() => received,
        };
        let published = match received {
            Ok(published) => published,
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                watcher.release();
                watcher.listening.push(listening::Event::Lagged { missed });
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                watcher.end();
                return;
            }
        };
        watcher.held.extend(told(&published));
        if !closes_a_chunk(&published.event) {
            if !watcher.held.is_empty() && watcher.let_go.is_none() {
                watcher.let_go = Some(Instant::now() + HOLD);
            }
            continue;
        }
        if !watcher.copy().await {
            watcher.end();
            return;
        }
    }
}

/// Until `at`, or forever when there is nothing to let go.
async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

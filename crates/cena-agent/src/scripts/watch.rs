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
//! A copy is taken as the agent's watcher takes one (`crate::characters`):
//! a fresh snapshot and its stream at each prompt, the old stream read up
//! to the new snapshot's fence first, so nothing between falls out.

use std::sync::Arc;
use std::time::Duration;

use cena_session::{Event, Frame, ObservedEvent, SessionObserver, Snapshot};
use tokio::sync::broadcast;
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
        Event::Sent { line, origin } => Some(listening::Event::Sent {
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

/// Keep what the session publishes for one runner, starting from
/// `snapshot` and its `events`, until stopped.
pub(super) async fn watch(
    watching: Watching,
    snapshot: Snapshot,
    mut events: broadcast::Receiver<ObservedEvent>,
) {
    let Watching {
        character,
        observer,
        atlas,
        listening,
        stop,
    } = watching;
    let mut whereabouts = Whereabouts::default();
    let copy = |snapshot: &Snapshot, whereabouts: &mut Whereabouts| -> Local {
        let map_room = whereabouts.locate(atlas.as_deref(), &snapshot.state);
        local(&character, snapshot, map_room)
    };
    let mut last = copy(&snapshot, &mut whereabouts);
    listening.push(listening::Event::State {
        cursor: snapshot.cursor,
        fields: changed_fields(None, &last),
    });
    let mut held: Vec<listening::Event> = Vec::new();
    let mut let_go: Option<Instant> = None;
    loop {
        let received = tokio::select! {
            () = stop.cancelled() => return,
            () = sleep_until(let_go) => {
                release(&listening, &mut held);
                let_go = None;
                continue;
            }
            received = events.recv() => received,
        };
        let published = match received {
            Ok(published) => published,
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                release(&listening, &mut held);
                listening.push(listening::Event::Lagged { missed });
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                release(&listening, &mut held);
                listening.close();
                return;
            }
        };
        held.extend(told(&published));
        if !closes_a_chunk(&published.event) {
            if !held.is_empty() && let_go.is_none() {
                let_go = Some(Instant::now() + HOLD);
            }
            continue;
        }
        let Some((snapshot, fresh)) = crate::characters::subscribe(&observer, &stop).await else {
            release(&listening, &mut held);
            listening.close();
            return;
        };
        // The events up to the new snapshot's fence are still in the old
        // receiver: they belong to this chunk.
        while let Ok(event) = events.try_recv() {
            if event.cursor > snapshot.cursor {
                break;
            }
            held.extend(told(&event));
        }
        events = fresh;
        let now = copy(&snapshot, &mut whereabouts);
        let fields = changed_fields(Some(&last), &now);
        if !fields.is_empty() {
            listening.push(listening::Event::State {
                cursor: snapshot.cursor,
                fields,
            });
        }
        last = now;
        release(&listening, &mut held);
        let_go = None;
    }
}

/// Pass on what was held, in order.
fn release(listening: &Listening, held: &mut Vec<listening::Event>) {
    for event in held.drain(..) {
        listening.push(event);
    }
}

/// Until `at`, or forever when there is nothing to let go.
async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

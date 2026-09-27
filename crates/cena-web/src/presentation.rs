//! A single bounded presentation pump. Native snapshots are coalesced, never
//! rebuilt by applying frames to a second `GameState`. Native cursor fences are
//! separate from the presentation sequence emitted to viewers.

mod hub;
mod pending;
#[cfg(test)]
mod tests;

pub(crate) use hub::{Hub, encode};

use crate::server::Viewed;
use cena_session::observation::retrying;
use cena_session::{Event, ObserveError, ObservedEvent, SessionObserver, Snapshot, State};
use pending::Pending;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

const MAX_HISTORY_LINES: usize = 256;
/// Retained Story, counted in **encoded** bytes -- see [`hub::line_bytes`].
const MAX_HISTORY_BYTES: usize = 96 * 1024;
pub(crate) const MAX_WIRE_BYTES: usize = 512 * 1024;

type Subscription = (Snapshot, broadcast::Receiver<ObservedEvent>);

pub(crate) async fn pump(observer: SessionObserver, viewed: Arc<Viewed>) -> std::io::Result<()> {
    project(|| observer.subscribe(), viewed).await
}

/// The pump over any source of subscriptions: [`pump`] passes the session's,
/// and a test passes one that fails on cue. A busy or slow owner is asked
/// again ([`retrying`] has why); only an owner gone ends it.
async fn project<F, Fut>(mut subscribe: F, shared: Arc<Viewed>) -> std::io::Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Subscription, ObserveError>>,
{
    let fatal = |error: ObserveError| {
        std::io::Error::other(format!("Session observation failed: {error:?}"))
    };
    let Some(result) = retrying(&shared.stop, &mut subscribe).await else {
        return Ok(());
    };
    let (initial, mut events) = result.map_err(fatal)?;
    let mut pending = Pending::new(&initial);
    if shared
        .hub
        .lock()
        .await
        .publish(&initial, Vec::new(), false)
        .is_err()
    {
        return Err(std::io::Error::other("Presentation sequence exhausted"));
    }
    // The hub page's cards are read from this view (`socket::serve_hub`).
    let _ = shared.changed.send(());
    let mut terminal = initial.lifecycle == State::Closed;
    let mut dirty = false;
    let mut ticking = initial.state.in_roundtime() == Some(true);
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let refresh = tokio::select! {
            () = shared.stop.cancelled() => return Ok(()),
            event = events.recv(), if !terminal => match event {
                Ok(event) => {
                    let immediate = event.generation != pending.generation
                        || matches!(event.event, Event::StateChanged(_) | Event::ConnectFailed { .. });
                    pending.observe(event);
                    dirty = true;
                    immediate
                }
                Err(broadcast::error::RecvError::Lagged(_)) => { pending.missing(); true }
                Err(broadcast::error::RecvError::Closed) => { terminal = true; true }
            },
            // While a roundtime runs this still asks every 100 ms, so the
            // seconds boundary is seen promptly; `Hub::publish` is what keeps
            // an unchanged answer off the wire.
            _ = interval.tick() => dirty || ticking,
        };
        if !refresh {
            continue;
        }
        let Some(result) = retrying(&shared.stop, &mut subscribe).await else {
            return Ok(());
        };
        // Stale state must never masquerade as an authoritative live view.
        let (snapshot, next) = result.map_err(fatal)?;
        pending.fence(&snapshot, &mut events);
        events = next;
        let lines: Vec<_> = pending.lines.drain(..).collect();
        // The hub's merged streams read the same lines this page shows.
        shared
            .merged
            .offer(shared.id, &shared.tag(), &shared.name, &lines);
        pending.bytes = 0;
        let gap = std::mem::take(&mut pending.gap);
        let mut hub = shared.hub.lock().await;
        if hub.publish(&snapshot, lines, gap).is_err() {
            return Err(std::io::Error::other("Presentation sequence exhausted"));
        }
        hub.alert(std::mem::take(&mut pending.alerts));
        drop(hub);
        let _ = shared.changed.send(());
        dirty = false;
        ticking = snapshot.state.in_roundtime() == Some(true);
        terminal = snapshot.lifecycle == State::Closed;
    }
}

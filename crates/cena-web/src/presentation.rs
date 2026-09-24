//! A single bounded presentation pump. Native snapshots are coalesced, never
//! rebuilt by applying frames to a second `GameState`. Native cursor fences are
//! separate from the presentation sequence emitted to viewers.

mod hub;
mod pending;
#[cfg(test)]
mod tests;

pub(crate) use hub::{Hub, encode};

use crate::server::Viewed;
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
const MAX_DRAIN: usize = 4096;
/// First wait after a retryable observation failure, doubled to the cap.
const OBSERVE_RETRY_FIRST: Duration = Duration::from_millis(100);
/// The longest wait between observation attempts: bounded, never a spin.
const OBSERVE_RETRY_CAP: Duration = Duration::from_secs(2);

/// Ask for a fresh observation, retrying the failures that are not an answer.
///
/// `None` when the viewer server is stopping; otherwise the subscription, or
/// the error that ends observation.
///
/// # Why this is not `?`
///
/// The pump used to map **every** [`ObserveError`] to a fatal I/O error. That
/// ended `WebServer::run`, disconnected every viewer, and was reported only
/// at shutdown -- for `Busy` (the owner's bounded inbox was momentarily
/// full) and `Timeout` (it did not answer within its budget), which
/// `cena_session::observation` documents as *retryable read failures, not
/// proof the owner died*. A busy owner under a login burst is exactly when a
/// viewer is most wanted.
///
/// So those two are retried with doubling backoff capped at
/// [`OBSERVE_RETRY_CAP`] -- bounded, never a spin -- and without a retry
/// limit: a timeout cannot prove death, and the pump's own stop token is
/// what ends the wait. Only `Closed`, the owner gone without a final
/// snapshot, is terminal. The last published view stays up meanwhile: that
/// is the stale view the pump already refuses to *replace* with a guess, not
/// a new one.
async fn observe<T, F, Fut>(
    stop: &tokio_util::sync::CancellationToken,
    mut subscribe: F,
) -> Option<Result<T, ObserveError>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ObserveError>>,
{
    let mut delay = OBSERVE_RETRY_FIRST;
    loop {
        let result = tokio::select! {
            () = stop.cancelled() => return None,
            result = subscribe() => result,
        };
        match result {
            Err(ObserveError::Busy | ObserveError::Timeout) => {}
            answered => return Some(answered),
        }
        tokio::select! {
            () = stop.cancelled() => return None,
            () = tokio::time::sleep(delay) => {}
        }
        delay = (delay * 2).min(OBSERVE_RETRY_CAP);
    }
}

type Subscription = (Snapshot, broadcast::Receiver<ObservedEvent>);

pub(crate) async fn pump(observer: SessionObserver, viewed: Arc<Viewed>) -> std::io::Result<()> {
    project(|| observer.subscribe(), viewed).await
}

/// The pump over any source of subscriptions: [`pump`] passes the session's,
/// and a test passes one that fails on cue.
async fn project<F, Fut>(mut subscribe: F, shared: Arc<Viewed>) -> std::io::Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Subscription, ObserveError>>,
{
    let fatal = |error: ObserveError| {
        std::io::Error::other(format!("Session observation failed: {error:?}"))
    };
    let Some(result) = observe(&shared.stop, &mut subscribe).await else {
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
        let Some(result) = observe(&shared.stop, &mut subscribe).await else {
            return Ok(());
        };
        // Stale state must never masquerade as an authoritative live view.
        let (snapshot, next) = result.map_err(fatal)?;
        pending.fence(&snapshot, &mut events);
        events = next;
        let lines = pending.lines.drain(..).collect();
        pending.bytes = 0;
        let gap = std::mem::take(&mut pending.gap);
        if shared
            .hub
            .lock()
            .await
            .publish(&snapshot, lines, gap)
            .is_err()
        {
            return Err(std::io::Error::other("Presentation sequence exhausted"));
        }
        dirty = false;
        ticking = snapshot.state.in_roundtime() == Some(true);
        terminal = snapshot.lifecycle == State::Closed;
    }
}

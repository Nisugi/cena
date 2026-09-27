//! One character's feed: follows its session and keeps its seat current,
//! waking the window when something changed (`plan/47` §4).
//!
//! The same shape as Despana's pump (`cena-web/src/presentation.rs`): a fresh
//! snapshot is asked for when an event arrives, at most every 100 ms unless
//! the connection changed, and every 100 ms while a roundtime runs, so the
//! seconds count down. Asking is `cena-session`'s ([`retrying`]), shared with
//! the pump rather than copied.

use std::future::Future;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use cena_session::observation::retrying;
use cena_session::{
    Event, Generation, ObserveError, ObservedEvent, SessionObserver, Snapshot, State,
};
use cena_ui::{LifecycleView, SessionCard, SessionView};
use tokio::sync::broadcast;

use crate::sessions::{Seat, Wake};

type Subscription = (Snapshot, broadcast::Receiver<ObservedEvent>);

/// Follow `observer`'s session for `seat`.
pub(crate) async fn feed(observer: SessionObserver, seat: Arc<Seat>, window: Wake) {
    follow(|| observer.subscribe(), seat, window).await;
}

/// [`feed`] over any source of subscriptions, so a test can script one:
/// until `seat` is detached or the session's owner is gone. The seat keeps
/// what was last seen either way.
pub(crate) async fn follow<F, Fut>(mut subscribe: F, seat: Arc<Seat>, window: Wake)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Subscription, ObserveError>>,
{
    let Some(Ok((snapshot, mut events))) = retrying(&seat.stop, &mut subscribe).await else {
        return;
    };
    let mut seen = Seen::of(&snapshot);
    show(&seat, &snapshot, &window);
    let mut terminal = snapshot.lifecycle == State::Closed;
    let mut ticking = snapshot.state.in_roundtime() == Some(true);
    let mut dirty = false;
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let refresh = tokio::select! {
            () = seat.stop.cancelled() => return,
            event = events.recv(), if !terminal => match event {
                Ok(event) => {
                    let immediate = event.generation != seen.generation
                        || matches!(event.event, Event::StateChanged(_) | Event::ConnectFailed { .. });
                    seen.observe(&event);
                    dirty = true;
                    immediate
                }
                Err(broadcast::error::RecvError::Lagged(_)) => true,
                Err(broadcast::error::RecvError::Closed) => { terminal = true; true }
            },
            _ = interval.tick() => dirty || ticking,
        };
        if !refresh {
            continue;
        }
        let Some(Ok((snapshot, next))) = retrying(&seat.stop, &mut subscribe).await else {
            return;
        };
        seen = Seen::of(&snapshot);
        events = next;
        show(&seat, &snapshot, &window);
        dirty = false;
        ticking = snapshot.state.in_roundtime() == Some(true);
        terminal = snapshot.lifecycle == State::Closed;
    }
}

/// Where the feed is in the session's stream.
struct Seen {
    cursor: u64,
    generation: Generation,
}

impl Seen {
    fn of(snapshot: &Snapshot) -> Self {
        Self {
            cursor: snapshot.cursor,
            generation: snapshot.generation,
        }
    }

    fn observe(&mut self, event: &ObservedEvent) {
        if event.cursor <= self.cursor {
            return;
        }
        self.cursor = event.cursor;
        self.generation = event.generation;
    }
}

/// Put `snapshot` on the seat's card, and wake the window.
fn show(seat: &Seat, snapshot: &Snapshot, window: &Wake) {
    let view = SessionView::project(
        &snapshot.state,
        &snapshot.triggers,
        lifecycle(snapshot),
        snapshot.state.game_time_now(),
    );
    {
        let mut card = seat.card.lock().unwrap_or_else(PoisonError::into_inner);
        *card = SessionCard::of(card.session.clone(), card.name.clone(), Some(&view));
    }
    window.wake();
}

/// How the session is connected, in the web hub's terms. The same mapping
/// as Despana's (`cena-web/src/presentation/hub.rs`, `lifecycle`): it joins
/// a `cena-session` type to a `cena-ui` one, so it can only live above both,
/// in each frontend.
pub(crate) fn lifecycle(snapshot: &Snapshot) -> LifecycleView {
    match snapshot.lifecycle {
        State::Ready => LifecycleView::Ready,
        State::Closed => LifecycleView::Closed { detail: None },
        State::Reconnecting => LifecycleView::Reconnecting {
            attempt: snapshot.retry.as_ref().map(|retry| retry.attempt),
            retry_delay_ms: snapshot
                .retry
                .as_ref()
                .map(|retry| u64::try_from(retry.delay.as_millis()).unwrap_or(u64::MAX)),
            detail: snapshot.retry.as_ref().map(|retry| retry.detail.clone()),
        },
        State::Connecting | State::Authenticating | State::Syncing => LifecycleView::Connecting,
    }
}

#[cfg(test)]
mod tests;

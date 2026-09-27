//! One character's feed: follows its session and keeps its seat current,
//! waking the window when something changed (`plan/47` §4).
//!
//! The same shape as Despana's pump (`cena-web/src/presentation.rs`): a fresh
//! snapshot is asked for when an event arrives, at most every 100 ms unless
//! the connection changed, and every 100 ms while a roundtime runs, so the
//! seconds count down. Asking and catching up are `cena-session`'s
//! ([`retrying`], [`catch_up`]), shared with the pump rather than copied.
//!
//! Between snapshots it hears each new event once ([`Ears`]): a line on a
//! merged stream goes to the hub's merged history.

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use cena_session::observation::{catch_up, retrying};
use cena_session::{
    Event, Generation, ObserveError, ObservedEvent, SessionObserver, Snapshot, State,
};
use cena_ui::{LifecycleView, MergedHistory, SessionCard, SessionView, painted, story_lines};
use tokio::sync::broadcast;

use crate::sessions::{Seat, Wake, lock};

type Subscription = (Snapshot, broadcast::Receiver<ObservedEvent>);

/// Follow `observer`'s session for `seat`, its merged lines into `merged`.
pub(crate) async fn feed(
    observer: SessionObserver,
    seat: Arc<Seat>,
    merged: Arc<Mutex<MergedHistory>>,
    window: Wake,
) {
    let ears = Ears { seat, merged };
    follow(|| observer.subscribe(), &ears, window).await;
}

/// What the feed does with each event it has not heard before.
pub(crate) struct Ears {
    /// Whose events they are.
    pub(crate) seat: Arc<Seat>,
    /// The merged streams every seat's lines go to.
    pub(crate) merged: Arc<Mutex<MergedHistory>>,
}

impl Ears {
    /// A line goes to the merged history, which keeps only the streams that
    /// merge ([`cena_ui::Merger::offer`]).
    fn hear(&self, event: &Event) {
        let Event::Line(line) = event else { return };
        let lines = story_lines(&line.stream, painted(line));
        lock(&self.merged).offer(
            Instant::now(),
            &self.seat.id.0.to_string(),
            &self.seat.tag(),
            &self.seat.name,
            &lines,
        );
    }
}

/// [`feed`] over any source of subscriptions, so a test can script one:
/// until the seat is detached or the session's owner is gone. The seat keeps
/// what was last seen either way.
pub(crate) async fn follow<F, Fut>(mut subscribe: F, ears: &Ears, window: Wake)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Subscription, ObserveError>>,
{
    let seat = &ears.seat;
    let Some(Ok((snapshot, mut events))) = retrying(&seat.stop, &mut subscribe).await else {
        return;
    };
    let mut seen = Seen::of(&snapshot);
    show(seat, &snapshot, &window);
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
                    if seen.fresh(&event) {
                        ears.hear(&event.event);
                    }
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
        // What the old receiver still holds up to the new snapshot was
        // published before it; the new receiver starts after.
        let (caught, _whole) = catch_up(&mut events, seen.cursor, snapshot.cursor);
        for event in &caught {
            ears.hear(&event.event);
        }
        seen = Seen::of(&snapshot);
        events = next;
        show(seat, &snapshot, &window);
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

    /// Whether `event` is new, moving past it if so.
    fn fresh(&mut self, event: &ObservedEvent) -> bool {
        if event.cursor <= self.cursor {
            return false;
        }
        self.cursor = event.cursor;
        self.generation = event.generation;
        true
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
        State::Closed => LifecycleView::Closed {
            detail: snapshot.stopped.clone(),
        },
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

//! A snapshot shown: on the seat, on its card, and the window woken.

use std::sync::{Arc, PoisonError};

use cena_session::{Snapshot, State};
use cena_ui::{LifecycleView, SessionCard, SessionView};

use crate::sessions::{Seat, Wake, lock};
use crate::story::inbox::Heard;

/// Put `snapshot` on the seat, and its card, and wake the window.
pub(super) fn show(seat: &Seat, snapshot: Snapshot, window: &Wake) {
    // The feed wakes every 100 ms while a roundtime runs, so the live `R>`
    // is settled within a tick of its end.
    let snapshot = Arc::new(snapshot);
    seat.tell_story(Heard::Settled(Arc::clone(&snapshot)));
    let view = SessionView::project(
        &snapshot.state,
        &snapshot.triggers,
        lifecycle(&snapshot),
        snapshot.state.game_time_now(),
    );
    seat.find_on_map(&snapshot.state);
    *lock(&seat.snapshot) = Some(snapshot);
    {
        let mut card = seat.card.lock().unwrap_or_else(PoisonError::into_inner);
        *card = SessionCard {
            game: std::mem::take(&mut card.game),
            ..SessionCard::of(card.session.clone(), card.name.clone(), Some(&view))
        };
    }
    window.wake();
}

/// How the session is connected, in the web hub's terms. The same mapping
/// as Despana's (`cena-web/src/presentation/hub.rs`, `lifecycle`): it joins
/// a `cena-session` type to a `cena-ui` one, so it can only live above both,
/// in each frontend.
pub(super) fn lifecycle(snapshot: &Snapshot) -> LifecycleView {
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

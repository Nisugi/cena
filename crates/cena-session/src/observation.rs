//! Detached read access. Requests are answered by the current state owner in
//! one synchronous turn, so a snapshot and its stream share an exact fence.

use crate::{Event, GameState, Generation, GenerationCell, SessionId, Snapshot, State};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

/// One occurrence, located in a session's ordered observation stream.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservedEvent {
    /// The session that published this occurrence.
    pub session: SessionId,
    /// The connection that published this occurrence.
    pub generation: Generation,
    /// Strictly increasing across reconnects, starting at one.
    pub cursor: u64,
    /// Native typed occurrence; frontends map this into their view vocabulary.
    pub event: Event,
}

/// Most recent retry decision, retained for subscribers joining during backoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryStatus {
    /// Attempt number, starting at one.
    pub attempt: u32,
    /// The scheduled delay, not a remaining-time countdown.
    pub delay: Duration,
    /// The connector's already-redacted reason for retrying.
    pub detail: String,
}

/// Why a fresh observation could not be obtained.
/// `Busy` and `Timeout` are retryable read failures, not proof the owner died.
/// Retry with bounded backoff; never spin or substitute a stale snapshot.
/// `Closed` is terminal for this observer: reacquire one from a new owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserveError {
    /// The bounded observation inbox is full; retry after yielding/backoff.
    Busy,
    /// The owner vanished without completing shutdown.
    Closed,
    /// The owner did not answer within the request budget; retry with backoff.
    Timeout,
}

type Subscription = (Snapshot, broadcast::Receiver<ObservedEvent>);
type Request = oneshot::Sender<Subscription>;

// A caller wait budget, not a measured game/network deadline or failure detector.
// The owner answers in a synchronous select-loop arm (no game round trip), so
// five seconds deliberately tolerates scheduling/load while bounding a hung
// owner's effect on a viewer. Expiry abandons only this read; it cannot stop the
// owner or prove death. The paused-clock unresponsive-owner test pins this policy.
const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Cloneable read-only access that survives consuming the session in `run`.
/// It has no command sender and cannot mutate the character.
#[derive(Clone, Debug)]
pub struct SessionObserver {
    requests: mpsc::Sender<Request>,
    final_snapshot: watch::Receiver<Option<Snapshot>>,
}

impl SessionObserver {
    /// Obtain a fresh snapshot and every event strictly after its cursor.
    /// On broadcast lag, discard the old receiver and call this again.
    ///
    /// # Errors
    /// Reports inbox saturation, an unresponsive owner, or an owner that
    /// disappeared without clean shutdown. After normal shutdown, returns
    /// the final `Closed` snapshot with an already closed event receiver.
    /// `Busy` and `Timeout` may be retried with bounded backoff; `Closed` cannot.
    pub async fn subscribe(&self) -> Result<Subscription, ObserveError> {
        let (reply, answer) = oneshot::channel();
        match self.requests.try_send(reply) {
            Ok(()) => match tokio::time::timeout(SUBSCRIBE_TIMEOUT, answer).await {
                Ok(Ok(subscription)) => Ok(subscription),
                Ok(Err(_)) => self.closed_snapshot(),
                Err(_) => Err(ObserveError::Timeout),
            },
            Err(mpsc::error::TrySendError::Full(_)) => Err(ObserveError::Busy),
            Err(mpsc::error::TrySendError::Closed(_)) => self.closed_snapshot(),
        }
    }

    fn closed_snapshot(&self) -> Result<Subscription, ObserveError> {
        let snapshot = self
            .final_snapshot
            .borrow()
            .clone()
            .ok_or(ObserveError::Closed)?;
        let (sender, events) = broadcast::channel(1);
        drop(sender);
        Ok((snapshot, events))
    }
}

/// Owned by the supervisor between connections, by the actor during one.
#[derive(Debug)]
pub(crate) struct ObservationRequests {
    pub(crate) requests: mpsc::Receiver<Request>,
    final_snapshot: watch::Sender<Option<Snapshot>>,
    observer: SessionObserver,
}

impl ObservationRequests {
    pub(crate) fn new() -> Self {
        let (requests, receiver) = mpsc::channel(32);
        let (final_snapshot, terminal) = watch::channel(None);
        Self {
            requests: receiver,
            final_snapshot,
            observer: SessionObserver {
                requests,
                final_snapshot: terminal,
            },
        }
    }

    pub(crate) fn observer(&self) -> SessionObserver {
        self.observer.clone()
    }

    pub(crate) fn finish(&mut self, snapshot: Snapshot) {
        self.final_snapshot.send_replace(Some(snapshot));
        self.requests.close();
        while self.requests.try_recv().is_ok() {}
    }
}

/// Both legacy and fenced streams share one publication point. Clones pass
/// ownership between the supervisor and actor; only that owner publishes.
#[derive(Clone, Debug)]
pub(crate) struct EventPublisher {
    legacy: broadcast::Sender<Event>,
    observed: broadcast::Sender<ObservedEvent>,
    cursor: Arc<AtomicU64>,
    generation: GenerationCell,
    session: SessionId,
    retry: Arc<Mutex<Option<RetryStatus>>>,
}

impl EventPublisher {
    pub(crate) fn new(bound: usize, generation: GenerationCell) -> Self {
        Self {
            legacy: broadcast::channel(bound).0,
            observed: broadcast::channel(bound).0,
            cursor: Arc::new(AtomicU64::new(0)),
            generation,
            session: SessionId::FIRST,
            retry: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn send(&self, event: Event) -> Result<usize, broadcast::error::SendError<Event>> {
        // Only control events touch this lock, never incoming frames. The
        // owner publishes the fact once and snapshots read that same fact.
        match &event {
            Event::ConnectFailed {
                attempt,
                delay,
                detail,
            } => {
                *self
                    .retry
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(RetryStatus {
                    attempt: *attempt,
                    delay: *delay,
                    detail: detail.clone(),
                });
            }
            Event::StateChanged(_) => {
                *self
                    .retry
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            }
            _ => {}
        }
        let cursor = self.cursor.fetch_add(1, Ordering::Relaxed) + 1;
        if self.observed.receiver_count() > 0 {
            let _ = self.observed.send(ObservedEvent {
                session: self.session,
                generation: self.generation.get(),
                cursor,
                event: event.clone(),
            });
        }
        self.legacy.send(event)
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.legacy.subscribe()
    }

    pub(crate) fn snapshot(&self, state: &GameState, lifecycle: State) -> Snapshot {
        Snapshot {
            session: self.session,
            generation: self.generation.get(),
            cursor: self.cursor.load(Ordering::Relaxed),
            state: state.clone(),
            lifecycle,
            retry: self
                .retry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        }
    }

    pub(crate) fn answer(&self, request: Request, state: &GameState, lifecycle: State) {
        if !request.is_closed() {
            let _ = request.send((self.snapshot(state, lifecycle), self.observed.subscribe()));
        }
    }
}

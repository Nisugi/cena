//! What a walk listens to. A session hands out its events two ways: plain,
//! to whoever subscribes before it starts, and **located** -- the same event
//! with its place in the stream -- to whoever joins while it runs
//! (`SessionObserver::subscribe`). A walk started from a typed command joins
//! late, so the driver listens to either, and sees one kind of event.
//!
//! # Falling behind
//!
//! A stream that lags has lost events, and a behavior folding its own state
//! from them has a state with holes in it: a room left, a hand emptied, a
//! connection dropped, none of which the game need say again. So a lag is
//! recorded ([`Heard::behind`]), and a behavior takes the state afresh
//! before it decides anything more ([`Heard::again`]): a snapshot from the
//! session and the stream from just after it, as `SessionObserver::subscribe`
//! is made for (*"On broadcast lag, discard the old receiver and call this
//! again"*). A stream joined without the session's observer cannot, and the
//! behavior stops rather than decide from the holes (the crate review of
//! 2026-09-28, R1).

use std::time::Duration;

use cena_session::{Event, ObserveError, ObservedEvent, SessionObserver, Snapshot};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

/// How long to wait before asking a busy session again for a fresh state.
const AGAIN_PAUSE: Duration = Duration::from_millis(100);
/// How many times to ask a busy or slow session before giving up.
const AGAIN_TRIES: u8 = 3;

/// A session's event stream, in whichever of its two forms the walk was given.
pub struct Heard {
    stream: Stream,
    /// The session, to take the state afresh from after a loss; none for a
    /// stream joined without it.
    observer: Option<SessionObserver>,
    /// Events were lost since the stream was last taken.
    behind: bool,
}

/// The two forms a session hands its events out in.
enum Stream {
    /// Subscribed before the session started: bare events.
    Plain(Receiver<Event>),
    /// Joined while it runs (`SessionObserver::subscribe`): each event with its
    /// place in the stream, which the walk drops.
    Located(Receiver<ObservedEvent>),
}

impl From<Receiver<Event>> for Heard {
    fn from(events: Receiver<Event>) -> Heard {
        Heard::of(Stream::Plain(events), None)
    }
}

impl From<Receiver<ObservedEvent>> for Heard {
    fn from(events: Receiver<ObservedEvent>) -> Heard {
        Heard::of(Stream::Located(events), None)
    }
}

impl Heard {
    fn of(stream: Stream, observer: Option<SessionObserver>) -> Heard {
        Heard {
            stream,
            observer,
            behind: false,
        }
    }

    /// `events`, joined through `observer`: a stream that, when it falls
    /// behind, can be taken afresh ([`Self::again`]).
    #[must_use]
    pub fn rejoinable(observer: SessionObserver, events: Receiver<ObservedEvent>) -> Heard {
        Heard::of(Stream::Located(events), Some(observer))
    }

    /// The next event, whichever kind the stream carries. A lag is recorded
    /// ([`Self::behind`]) as well as returned.
    ///
    /// # Errors
    ///
    /// The stream lagged or closed (`RecvError`).
    pub async fn recv(&mut self) -> Result<Event, RecvError> {
        let next = match &mut self.stream {
            Stream::Plain(events) => events.recv().await,
            Stream::Located(events) => events.recv().await.map(|located| located.event),
        };
        if matches!(next, Err(RecvError::Lagged(_))) {
            self.behind = true;
        }
        next
    }

    /// An event already waiting, or why not. A lag is recorded
    /// ([`Self::behind`]) as well as returned.
    ///
    /// # Errors
    ///
    /// Nothing waiting, lagged, or closed (`TryRecvError`).
    pub fn try_recv(&mut self) -> Result<Event, TryRecvError> {
        let next = match &mut self.stream {
            Stream::Plain(events) => events.try_recv(),
            Stream::Located(events) => events.try_recv().map(|located| located.event),
        };
        if matches!(next, Err(TryRecvError::Lagged(_))) {
            self.behind = true;
        }
        next
    }

    /// A second listener from this point on: what a hunt hands the walk it
    /// runs inside itself, keeping its own stream to fold meanwhile. It can be
    /// taken afresh as this one can.
    #[must_use]
    pub fn resubscribe(&self) -> Heard {
        let stream = match &self.stream {
            Stream::Plain(events) => Stream::Plain(events.resubscribe()),
            Stream::Located(events) => Stream::Located(events.resubscribe()),
        };
        Heard::of(stream, self.observer.clone())
    }

    /// Whether events were lost since the stream was last taken: what was
    /// folded from it is not the game's.
    #[must_use]
    pub fn behind(&self) -> bool {
        self.behind
    }

    /// The session's state afresh, and this stream from just after it, the
    /// loss forgotten; `None` when there is no session to ask
    /// ([`Self::rejoinable`] was not how it was joined), or it will not
    /// answer.
    pub async fn again(&mut self) -> Option<Snapshot> {
        let observer = self.observer.clone()?;
        for _ in 0..AGAIN_TRIES {
            match observer.subscribe().await {
                Ok((snapshot, events)) => {
                    self.stream = Stream::Located(events);
                    self.behind = false;
                    return Some(snapshot);
                }
                Err(ObserveError::Busy | ObserveError::Timeout) => {
                    tokio::time::sleep(AGAIN_PAUSE).await;
                }
                Err(ObserveError::Closed) => return None,
            }
        }
        None
    }
}

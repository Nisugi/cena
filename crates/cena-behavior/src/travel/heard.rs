//! What a walk listens to. A session hands out its events two ways: plain,
//! to whoever subscribes before it starts, and **located** -- the same event
//! with its place in the stream -- to whoever joins while it runs
//! (`SessionObserver::subscribe`). A walk started from a typed command joins
//! late, so the driver listens to either, and sees one kind of event.

use cena_session::{Event, ObservedEvent};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

/// A session's event stream, in whichever of its two forms the walk was given.
pub enum Heard {
    /// Subscribed before the session started: bare events.
    Plain(Receiver<Event>),
    /// Joined while it runs (`SessionObserver::subscribe`): each event with its
    /// place in the stream, which the walk drops.
    Located(Receiver<ObservedEvent>),
}

impl From<Receiver<Event>> for Heard {
    fn from(events: Receiver<Event>) -> Heard {
        Heard::Plain(events)
    }
}

impl From<Receiver<ObservedEvent>> for Heard {
    fn from(events: Receiver<ObservedEvent>) -> Heard {
        Heard::Located(events)
    }
}

impl Heard {
    /// The next event, whichever kind the stream carries.
    ///
    /// # Errors
    ///
    /// The stream lagged or closed (`RecvError`).
    pub async fn recv(&mut self) -> Result<Event, RecvError> {
        match self {
            Heard::Plain(events) => events.recv().await,
            Heard::Located(events) => events.recv().await.map(|located| located.event),
        }
    }

    /// An event already waiting, or why not.
    ///
    /// # Errors
    ///
    /// Nothing waiting, lagged, or closed (`TryRecvError`).
    pub fn try_recv(&mut self) -> Result<Event, TryRecvError> {
        match self {
            Heard::Plain(events) => events.try_recv(),
            Heard::Located(events) => events.try_recv().map(|located| located.event),
        }
    }

    /// A second listener from this point on: what a hunt hands the walk it
    /// runs inside itself, keeping its own stream to fold meanwhile.
    #[must_use]
    pub fn resubscribe(&self) -> Heard {
        match self {
            Heard::Plain(events) => Heard::Plain(events.resubscribe()),
            Heard::Located(events) => Heard::Located(events.resubscribe()),
        }
    }
}

//! What a walk listens to. A session hands out its events two ways: plain,
//! to whoever subscribes before it starts, and **located** -- the same event
//! with its place in the stream -- to whoever joins while it runs
//! (`SessionObserver::subscribe`). A walk started from a typed command joins
//! late, so the driver listens to either, and sees one kind of event.

use cena_session::{Event, ObservedEvent};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

pub enum Heard {
    Plain(Receiver<Event>),
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
    pub(super) async fn recv(&mut self) -> Result<Event, RecvError> {
        match self {
            Heard::Plain(events) => events.recv().await,
            Heard::Located(events) => events.recv().await.map(|located| located.event),
        }
    }

    pub(super) fn try_recv(&mut self) -> Result<Event, TryRecvError> {
        match self {
            Heard::Plain(events) => events.try_recv(),
            Heard::Located(events) => events.try_recv().map(|located| located.event),
        }
    }
}

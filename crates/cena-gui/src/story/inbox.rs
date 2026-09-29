//! What a feed has for its story while the play window is drawing it.
//!
//! The window holds the story for the whole of a draw, and the feed took the
//! same lock for every event it heard, so the feed waited on each frame and
//! the session's events waited on the feed (the crate review of 2026-09-28).
//! Now the feed puts what it heard here and hands it over only when the story
//! is free; while a draw holds it, the feed goes on, and whoever takes the
//! story next -- the feed with its next event, or the window's next frame --
//! puts it in first. The order is the order heard, because both hand over
//! only while holding the story.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use cena_session::{ObservedEvent, Snapshot};

use super::Story;

/// One thing the feed heard for the story.
pub(crate) enum Heard {
    /// An event, with the character as it was when heard.
    Event(ObservedEvent, Option<Arc<Snapshot>>),
    /// Events were lost: the feed fell behind.
    Missed,
    /// A snapshot: a live prompt may be settled by it.
    Settled(Arc<Snapshot>),
}

/// What the feed heard and the story has not had yet, oldest first.
#[derive(Default)]
pub(crate) struct Inbox(Mutex<VecDeque<Heard>>);

impl Inbox {
    /// Keep `heard` for the story.
    pub(crate) fn put(&self, heard: Heard) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(heard);
    }

    /// Everything kept, into `story`, in the order heard.
    pub(crate) fn hand_to(&self, story: &mut Story) {
        let waiting = std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner));
        for heard in waiting {
            match heard {
                Heard::Event(event, state) => {
                    story.hear(&event, state.as_ref().map(|snapshot| &snapshot.state));
                }
                Heard::Missed => story.missed(),
                Heard::Settled(snapshot) => story.settle(&snapshot.state),
            }
        }
    }
}

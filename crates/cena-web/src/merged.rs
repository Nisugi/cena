//! The hub page's merged streams (`plan/29` step 5d): thoughts, speech,
//! logons, deaths and announcements from every character, each line once and
//! tagged with who received it. The rules and the history are `cena_ui`'s
//! ([`MergedHistory`]), shared with the window's hub; this is the one place
//! every session's lines are offered to it here, and what tells the open hub
//! pages.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use cena_session::SessionId;
use cena_ui::{MergedHistory, MergedLine, ServerMessage, StoryLine, WIRE_VERSION};
use tokio::sync::broadcast;

use crate::presentation::encode;

/// The merged history and the hub pages listening to it.
pub(crate) struct MergedFeed {
    history: Mutex<MergedHistory>,
    pub(crate) updates: broadcast::Sender<Arc<str>>,
}

impl MergedFeed {
    pub(crate) fn new() -> Self {
        Self {
            history: Mutex::new(MergedHistory::new()),
            updates: broadcast::channel(64).0,
        }
    }

    /// Offer the lines `session` -- tagged `tag`, the character named `name`
    /// -- just published. Those on a merged stream reach every hub page: new,
    /// or as an earlier line gaining this character.
    pub(crate) fn offer(&self, session: SessionId, tag: &str, name: &str, lines: &[StoryLine]) {
        let merged = self
            .history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .offer(Instant::now(), &session.0.to_string(), tag, name, lines);
        if merged.is_empty() {
            return;
        }
        let message = ServerMessage::Merged {
            version: WIRE_VERSION,
            lines: merged,
        };
        if let Ok(encoded) = encode(&message) {
            let _ = self.updates.send(encoded);
        }
    }

    /// Everything a hub page opening now is shown.
    pub(crate) fn history(&self) -> Vec<MergedLine> {
        self.history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .lines()
            .cloned()
            .collect()
    }
}

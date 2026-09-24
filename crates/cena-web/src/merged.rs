//! The hub page's merged streams (`plan/29` step 5d): thoughts, speech,
//! logons, deaths and announcements from every character, each line once and
//! tagged with who received it. The rules are `cena_ui::Merger`'s; this is
//! the one shared place every session's lines are offered to it, and the
//! history a hub page is sent when it opens.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use cena_session::SessionId;
use cena_ui::{MergedLine, Merger, ServerMessage, StoryLine, WIRE_VERSION};
use tokio::sync::broadcast;

use crate::presentation::encode;

/// Merged lines a hub page opening now is shown.
const MAX_MERGED_HISTORY: usize = 200;

/// The merger, its history and the hub pages listening to it.
pub(crate) struct MergedFeed {
    merger: Mutex<Merger>,
    history: Mutex<VecDeque<MergedLine>>,
    pub(crate) updates: broadcast::Sender<Arc<str>>,
}

impl MergedFeed {
    pub(crate) fn new() -> Self {
        Self {
            merger: Mutex::new(Merger::new()),
            history: Mutex::new(VecDeque::new()),
            updates: broadcast::channel(64).0,
        }
    }

    /// Offer the lines `session` -- tagged `tag` -- just published. Those on a
    /// merged stream reach every hub page: new, or as an earlier line gaining
    /// this character.
    pub(crate) fn offer(&self, session: SessionId, tag: &str, lines: &[StoryLine]) {
        let now = Instant::now();
        let id = session.0.to_string();
        let merged: Vec<MergedLine> = {
            let mut merger = self.merger.lock().unwrap_or_else(PoisonError::into_inner);
            lines
                .iter()
                .filter_map(|line| merger.offer(now, &id, tag, line))
                .collect()
        };
        if merged.is_empty() {
            return;
        }
        {
            let mut history = self.history.lock().unwrap_or_else(PoisonError::into_inner);
            for line in &merged {
                match history.iter_mut().find(|kept| kept.id == line.id) {
                    Some(kept) => kept.clone_from(line),
                    None => history.push_back(line.clone()),
                }
            }
            while history.len() > MAX_MERGED_HISTORY {
                history.pop_front();
            }
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
            .iter()
            .cloned()
            .collect()
    }
}

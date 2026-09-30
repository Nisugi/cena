//! What Hydra says to the player: an answer in the story, its own news in
//! its own pane, and a trigger's banners, each up for a while. Moved out of
//! `story.rs` when the story's own rules took it to its cap.

use std::time::Instant;

use cena_session::{Notice, NoticeKind};

use super::{ALERT_FOR, MAX_ALERTS, MAX_SAID, Story};

impl Story {
    /// Hydra says `notice` to the player: an answer to something the
    /// player did in the story, among the game's lines, and the rest in
    /// Hydra's own pane (the author, 2026-09-29, `plan/49`).
    pub(crate) fn tell(&mut self, notice: Notice) {
        if notice.kind == NoticeKind::Debug {
            return;
        }
        if notice.answer {
            self.push(super::Shown::Said(notice));
            return;
        }
        self.said.push_back(notice);
        self.told += 1;
        while self.said.len() > MAX_SAID {
            self.said.pop_front();
        }
    }

    /// A trigger's banner `text`, up from now.
    pub(super) fn alert(&mut self, text: &str) {
        self.alerts.push_back((Instant::now(), text.to_owned()));
        while self.alerts.len() > MAX_ALERTS {
            self.alerts.pop_front();
        }
    }

    /// The banners still up at `now`, oldest first.
    pub(crate) fn alerts_at(&self, now: Instant) -> impl Iterator<Item = &str> {
        self.alerts
            .iter()
            .filter(move |(at, _)| now.saturating_duration_since(*at) < ALERT_FOR)
            .map(|(_, text)| text.as_str())
    }
}

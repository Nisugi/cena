//! Each stream's own lines, kept for a widget of that stream (`plan/49`
//! Stage B step 4): thoughts, speech, arrivals, and any other the game
//! sends, painted by the character's triggers as the story's are. Kept
//! whether the story shows them or not, since the game drops some streams
//! from the story when their window is closed -- speech is main's copy --
//! and a widget of one is that window, open.

use std::collections::{BTreeMap, VecDeque};

use cena_ui::StyledRun;

use super::Stamp;

/// Lines a stream keeps, newest last.
pub(crate) const MAX_STREAM: usize = 500;

/// Every stream heard, by id.
#[derive(Debug, Default)]
pub(crate) struct Streams {
    by_id: BTreeMap<String, Kept>,
}

/// One stream's lines.
#[derive(Debug, Default)]
pub(crate) struct Kept {
    /// Its lines, oldest first, each with when it arrived.
    pub(crate) lines: VecDeque<(Stamp, Vec<StyledRun>)>,
    /// Lines heard, ever: a tab not showing counts what came since.
    pub(crate) heard: u64,
}

impl Streams {
    /// Keep `lines`, heard on stream `id`.
    pub(crate) fn hear(&mut self, id: &str, lines: impl IntoIterator<Item = Vec<StyledRun>>) {
        let kept = self.by_id.entry(id.to_owned()).or_default();
        for line in lines {
            kept.lines.push_back((Stamp::now(), line));
            kept.heard += 1;
            while kept.lines.len() > MAX_STREAM {
                kept.lines.pop_front();
            }
        }
    }

    /// Stream `id`'s lines, once it has sent any.
    pub(crate) fn get(&self, id: &str) -> Option<&Kept> {
        self.by_id.get(id)
    }

    /// Every line of stream `id` said to have arrived `at`, as a test sets
    /// the clock.
    #[cfg(test)]
    pub(crate) fn stamp_all(&mut self, id: &str, at: Stamp) {
        let kept = self.by_id.entry(id.to_owned()).or_default();
        for (stamp, _) in &mut kept.lines {
            *stamp = at;
        }
    }

    /// Every stream heard, by id.
    pub(crate) fn ids(&self) -> impl Iterator<Item = &str> {
        self.by_id.keys().map(String::as_str)
    }
}

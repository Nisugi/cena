//! What was sent from a play window, for up and down and the repeating keys
//! (`plan/52` step 3), kept per character so a restart does not lose it, as
//! Vellum keeps it (`reference/VellumFE/src/frontend/gui/app/command_input.rs:17-40`).
//! Walking back keeps the line being typed, and walking forward past the
//! newest line gives it back.

use std::path::{Path, PathBuf};

/// Lines kept.
pub(super) const MAX: usize = 100;

/// What was sent, oldest first, and where up and down have reached.
#[derive(Debug, Default)]
pub(super) struct History {
    lines: Vec<String>,
    /// Where up and down have reached: `None` at the line being typed.
    back: Option<usize>,
    /// The line being typed when up was first pressed, given back by down.
    draft: String,
    /// The folder and the file the lines are kept in; `None` keeps them
    /// for this window only.
    kept: Option<(PathBuf, PathBuf)>,
    /// Why the lines could not be kept, until they can.
    unsaved: Option<String>,
}

impl History {
    /// `character`'s history on `instance`, kept in `dir`: what it read
    /// there, or none. A file that cannot be read starts it empty, and the
    /// next line sent writes it anew.
    pub(super) fn kept(dir: &Path, instance: Option<&str>, character: &str) -> Self {
        let file = crate::layout::character_file(dir, instance, character).with_extension("txt");
        let mut lines: Vec<String> = std::fs::read_to_string(&file)
            .map(|text| {
                text.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        lines.drain(..lines.len().saturating_sub(MAX));
        Self {
            lines,
            kept: Some((dir.to_path_buf(), file)),
            ..Self::default()
        }
    }

    /// `line`, sent: kept unless it is the newest again, and written to its
    /// file when it has one. Up and down start again from the line typed.
    pub(super) fn keep(&mut self, line: &str) {
        self.reset();
        if line.contains('\n') || self.lines.last().is_some_and(|last| last == line) {
            return;
        }
        self.lines.push(line.to_owned());
        if self.lines.len() > MAX {
            self.lines.remove(0);
        }
        if let Some((dir, file)) = &self.kept {
            let mut text = self.lines.join("\n");
            text.push('\n');
            self.unsaved = cena_session::store::save_text(dir, file, &text)
                .err()
                .map(|why| why.to_string());
        }
    }

    /// The line sent `back` lines ago, 1 being the newest.
    pub(super) fn ago(&self, back: usize) -> Option<String> {
        self.lines
            .len()
            .checked_sub(back)
            .and_then(|at| self.lines.get(at).cloned())
    }

    /// One step back (`up`) or forward from `input`, which becomes the line
    /// reached. The first step back keeps what was typed, and the step
    /// forward past the newest line gives it back.
    pub(super) fn walk(&mut self, up: bool, input: &mut String) {
        let last = self.lines.len().checked_sub(1);
        let reached = match (self.back, up) {
            (None, true) => last,
            (Some(at), true) => Some(at.saturating_sub(1)),
            (Some(at), false) if Some(at) < last => Some(at + 1),
            (_, false) => None,
        };
        if self.back.is_none() && reached.is_some() {
            self.draft = std::mem::take(input);
        }
        *input = match reached {
            Some(at) => self.lines.get(at).cloned().unwrap_or_default(),
            None if self.back.is_some() => std::mem::take(&mut self.draft),
            None => std::mem::take(input),
        };
        self.back = reached;
    }

    /// Up and down start again from the line typed, which is kept as it is.
    pub(super) fn reset(&mut self) {
        self.back = None;
        self.draft.clear();
    }

    /// Why the lines could not be kept, if they could not.
    pub(super) fn unsaved(&self) -> Option<&str> {
        self.unsaved.as_deref()
    }
}

impl super::Play {
    /// What is typed, sent: taken from the input, and kept in the history
    /// unless it is the last line again. Nothing for an empty line.
    pub(super) fn enter(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.input);
        if line.trim().is_empty() {
            self.history.reset();
            return None;
        }
        self.history.keep(&line);
        Some(line)
    }

    /// Keep what is sent in `dir`, by the character's game and name, and
    /// take up what was kept there: the app's windows do, and a test's
    /// need not.
    #[must_use]
    pub(crate) fn keeping_history(mut self, dir: &std::path::Path) -> Self {
        self.history = History::kept(dir, self.instance, &self.name);
        self
    }
}

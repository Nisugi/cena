//! A story's prompts, and what the player typed after one, shown as
//! `VellumFE` shows them (`core/messages/element.rs`, the `Prompt` arm): a
//! prompt after the story has had a line since the last, or when it changed
//! (`R>` to `>` as roundtime ends), never twice in a row for nothing; what
//! the player types echoed after the last prompt shown, `>look`
//! (`core/app_core/commands.rs`, the echo). The author, 2026-09-28: *"there
//! is no prompt"*.

use super::{Shown, Story};

impl Story {
    /// The player typed `line` here.
    pub(crate) fn typed(&mut self, line: &str) {
        self.push(Shown::Typed {
            prompt: self.prompt.clone().unwrap_or_else(|| ">".to_owned()),
            line: line.to_owned(),
        });
    }

    /// The game's prompt `text`: shown when the story has had a line since
    /// the last, or when it changed; an empty one never.
    pub(super) fn prompted(&mut self, text: &str) {
        let changed = self.prompt.as_deref().unwrap_or(">").trim() != text.trim();
        if (self.since_prompt || changed) && !text.trim().is_empty() {
            self.prompt = Some(text.to_owned());
            self.push(Shown::Prompt(text.to_owned()));
        }
        self.since_prompt = false;
    }
}

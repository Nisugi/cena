//! A story's prompts, and what the player typed after one, shown as
//! `VellumFE` shows them (`core/messages/element.rs`, the `Prompt` arm): a
//! prompt after the story has had a line with something to read since the
//! last, or when it changed (`R>` to `>` as roundtime ends), never twice in
//! a row for nothing; what the player types echoed after the last prompt
//! shown, `>look` (`core/app_core/commands.rs`, the echo). The author,
//! 2026-09-28: *"there is no prompt"*; and 2026-09-29: *"We want to throw
//! away prompts that come in and do not have any visible text to show unless
//! the prompt holds a change."*
//!
//! The game sends a prompt with every chunk: one of effects alone, a
//! thought, a change of indicator. A chunk with nothing for the story to
//! read earns no prompt (a blank line counts as nothing, [`super::visible`]);
//! nor does one whose lines another widget shows, which is decided where the
//! story is drawn (`crate::widget`'s `lines.rs`, `Prompts`). A prompt is
//! taken from [`cena_session::Event::Prompt`], which lands after the lines
//! it ends even while the player's Lich runs.
//!
//! Where `VellumFE` stops short, the live prompt -- the last line, nothing
//! printed after it -- drops its `R` once roundtime has run out
//! ([`Story::settle`]). The author, the same day: *"It shows the R> to indicate
//! roundtime, but roundtime ending doesn't send a prompt to update that ...
//! because the game also doesn't send a message saying hey you're out of
//! roundtime, it tells you up front exactly when it will end."* That is
//! `plan/15` §2a.1: a prompt gives onset only. The end is the model's clock
//! against `<roundTime>`.

use cena_session::GameState;
use cena_ui::StyledRun;

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

    /// The live prompt as the clock has it now: `R>` becomes `>` once
    /// `state`'s roundtime has run out. Only `R`: the other letters end with
    /// an action, and an action brings a prompt. The scrollback's prompts stay
    /// as printed, each true when it was; an unknown clock changes nothing.
    /// What a new prompt is compared with, and an echo follows, is the prompt
    /// as shown.
    pub(crate) fn settle(&mut self, state: &GameState) {
        if state.in_roundtime() != Some(false) {
            return;
        }
        let Some((_, Shown::Prompt(text))) = self.lines.back_mut() else {
            return;
        };
        if !text.contains('R') {
            return;
        }
        *text = text.replacen('R', "", 1);
        self.prompt = Some(text.clone());
    }
}

/// Whether a line has anything to read: a blank one earns no prompt, as in
/// `VellumFE`, which counts a line toward the next prompt only for text that
/// is not whitespace (`core/messages/flush_line.rs`, `chunk_has_main_text`).
pub(crate) fn visible(runs: &[StyledRun]) -> bool {
    runs.iter().any(|run| !run.text.trim().is_empty())
}

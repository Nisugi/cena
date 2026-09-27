//! A character's play window (`plan/47` step 4): its story, one command
//! input that sends on this character, and what a player glances at --
//! connection, roundtime and cast time, hands, vitals, the room -- with
//! Hydra's own messages in their own pane, never in the game's text.
//!
//! The author: *"in my head you have one command input period. sending a
//! command in it sends a command on that character."* One window per
//! character, so no window mixes two characters' story (`plan/29` §5a R2).
//! Closing it leaves the character running headless; the hub opens it again.
//!
//! It draws from the character's `GameState` directly, as the author chose
//! for the native GUI (`plan/28` §7b, *"Third option is the one"*), with the
//! room's player names painted by the character's triggers as Despana paints
//! them ([`cena_ui::room_player`]). Its panes -- story, vitals, room, Hydra
//! -- drag, resize and snap inside it, and their layout is kept by the
//! character's name (step 6, `panes.rs`, `crate::layout`).
//!
//! **The focus rule**, `VellumFE`'s (`reference/VellumFE/src/frontend/gui/app.rs:3426`),
//! for the author's complaint: a click nothing else took returns the
//! keyboard to the command input, so the player can type without clicking it.

mod draw;
mod panes;

use std::path::PathBuf;
use std::time::Instant;

use cena_session::Snapshot;
use cena_ui::{HuntView, LifecycleView};
use egui::Id;

use crate::layout::{GRID, Layout};
use crate::story::Story;
use panes::Engaged;

/// What a play window asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    /// Send this line on the character, as typed: Hydra's command line
    /// first, then the game.
    Send(String),
    /// Stop everything Hydra is doing on the character (`;stop`).
    Stop,
    /// Read the keybinds file again.
    ReloadKeys,
}

/// What a play window shows this frame.
pub(crate) struct PlayView<'a> {
    /// The character.
    pub(crate) name: &'a str,
    /// How it is connected.
    pub(crate) lifecycle: &'a LifecycleView,
    /// What it knows, once its feed has seen anything.
    pub(crate) snapshot: Option<&'a Snapshot>,
    /// Its story, messages and banners.
    pub(crate) story: &'a Story,
    /// Now, for which banners are still up.
    pub(crate) now: Instant,
    /// What its hunt is doing, when one runs.
    pub(crate) hunt: Option<&'a HuntView>,
    /// `NumLock`, once a numpad press has shown it.
    pub(crate) numlock: Option<bool>,
    /// What the keybinds file bound, and what is wrong in it.
    pub(crate) keys: &'a [String],
}

/// A play window's own state, which outlives a frame.
#[derive(Debug)]
pub(crate) struct Play {
    /// Which session: every id in the window is its own.
    session: u32,
    /// The character, whose name its layout is kept by.
    name: String,
    /// Where layouts are kept; `None`, and nothing is saved.
    layouts: Option<PathBuf>,
    /// Where its panes sit: saved, or fitted to the window when first drawn.
    layout: Option<Layout>,
    /// The panes let go for a drag or resize under way.
    engaged: Vec<Engaged>,
    /// Why the layout could not be saved, until it can.
    unsaved: Option<String>,
    /// The line being typed.
    input: String,
    /// What was sent, oldest first, for up and down.
    history: Vec<String>,
    /// Where up and down have reached in the history.
    back: Option<usize>,
}

/// Lines of history kept for up and down.
const MAX_HISTORY: usize = 100;

impl Play {
    /// A play window for session `session`, the character `name`, with its
    /// layout from `layouts` when one was saved there.
    pub(crate) fn new(session: u32, name: &str, layouts: Option<PathBuf>) -> Self {
        let layout = layouts.as_deref().and_then(|dir| Layout::load(dir, name));
        Self {
            session,
            name: name.to_owned(),
            layouts,
            layout,
            engaged: Vec::new(),
            unsaved: None,
            input: String::new(),
            history: Vec::new(),
            back: None,
        }
    }

    /// The command input's id.
    pub(crate) fn input_id(&self) -> Id {
        Id::new(("play-input", self.session))
    }

    /// Draw the window into `ui` -- a viewport's whole area -- and return
    /// what the player asked for, if anything.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, view: &PlayView<'_>) -> Option<Asked> {
        let mut asked = None;
        let session = self.session;
        let mut grid = self.layout.as_ref().map_or(GRID, |layout| layout.grid);
        let unsaved = self.unsaved.clone();
        let top = egui::Panel::top(Id::new(("play-top", session)))
            .show(ui, |ui| draw::top(ui, view, &mut grid, unsaved.as_deref()))
            .inner;
        let mut changed = false;
        match top {
            Some(draw::Top::Stop) => asked = Some(Asked::Stop),
            Some(draw::Top::ReloadKeys) => asked = Some(Asked::ReloadKeys),
            Some(draw::Top::Fit) => {
                self.layout = None;
                changed = true;
            }
            Some(draw::Top::Grid) => {
                if let Some(layout) = &mut self.layout {
                    layout.grid = grid;
                    changed = true;
                }
            }
            None => {}
        }
        egui::Panel::bottom(Id::new(("play-input-panel", session))).show(ui, |ui| {
            if let Some(line) = self.input(ui) {
                asked = Some(Asked::Send(line));
            }
        });
        changed |= self.arrange(ui, view);
        if changed {
            self.save();
        }
        // The focus rule: nothing holds the keyboard, so the input takes it.
        if ui.ctx().memory(|memory| memory.focused().is_none()) {
            let id = self.input_id();
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
        }
        asked
    }

    /// Keep the layout under the character's name, or say why it could not be.
    fn save(&mut self) {
        let (Some(dir), Some(layout)) = (&self.layouts, &self.layout) else {
            return;
        };
        self.unsaved = layout
            .save(dir, &self.name)
            .err()
            .map(|why| why.to_string());
    }

    /// The command input: Enter sends, up and down walk what was sent.
    fn input(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let id = self.input_id();
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.input)
                .id(id)
                .hint_text("Type a command")
                .desired_width(f32::INFINITY),
        );
        if response.has_focus() {
            let (up, down) = ui.input(|input| {
                (
                    input.key_pressed(egui::Key::ArrowUp),
                    input.key_pressed(egui::Key::ArrowDown),
                )
            });
            if up {
                self.walk(true);
            } else if down {
                self.walk(false);
            }
        }
        let entered =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if !entered {
            return None;
        }
        response.request_focus();
        let line = std::mem::take(&mut self.input);
        self.back = None;
        if line.trim().is_empty() {
            return None;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        Some(line)
    }

    /// One step back (`up`) or forward through what was sent.
    fn walk(&mut self, up: bool) {
        let last = self.history.len().checked_sub(1);
        self.back = match (self.back, up) {
            (None, true) => last,
            (Some(at), true) => Some(at.saturating_sub(1)),
            (Some(at), false) if Some(at) < last => Some(at + 1),
            (_, false) => None,
        };
        self.input = self
            .back
            .and_then(|at| self.history.get(at).cloned())
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests;

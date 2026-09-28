//! The settings menu's *Keys* page (`plan/50` §7 step 2): the keybinds, one
//! set for every character (§6 item 1), each key set by pressing it rather
//! than typing its name. A key that types is refused, as the file refuses
//! it. While the page waits for a key, the window opens the fork's channel
//! to every numpad key and hands the press here, so a numpad key is never
//! taken for its digit.

use std::collections::HashMap;

use super::{Chord, winit_name};

/// A change the Keys page asks for, a key written as the file writes it:
/// `Ctrl+F1`, `Numpad8`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyChange {
    /// `key` sends `line`.
    Bind {
        /// The key.
        key: String,
        /// The line it sends, as if typed.
        line: String,
        /// The key the line moved from, which sends nothing now.
        was: Option<String>,
    },
    /// `key` sends nothing.
    Unbind(String),
    /// Whether the numpad sends its keys with `NumLock` on too.
    NumpadAlways(bool),
}

/// What the Keys page draws from this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeysView<'a> {
    /// Every key bound and the line it sends, the key written as the file
    /// writes it.
    pub bound: &'a [(String, String)],
    /// Whether the numpad sends its keys with `NumLock` on too.
    pub numpad_always: bool,
    /// What the window says of the file: where it is and how many keys it
    /// binds, then what in it is wrong.
    pub said: &'a [String],
    /// A numpad key pressed this frame while the page waits for a key,
    /// written as the file writes it.
    pub numpad: Option<&'a str>,
}

/// What the page is waiting for a key for.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Waiting {
    /// A new binding.
    New,
    /// To move this key's line to another key.
    Move(String),
}

/// The Keys page's state, which outlives a frame.
#[derive(Debug, Default)]
pub(crate) struct KeysPage {
    waiting: Option<Waiting>,
    /// A key pressed for a new binding, and the line typed for it so far.
    adding: Option<(String, String)>,
    /// The new binding's field is to take the focus.
    focus_new: bool,
    /// What is typed into a bound key's line while its field has the focus.
    typed: HashMap<String, String>,
}

impl KeysPage {
    /// Whether the page is waiting for a key to be pressed.
    pub(crate) fn waiting(&self) -> bool {
        self.waiting.is_some()
    }

    /// Draw the page over `view`. What it asks, when anything; why a key
    /// was refused goes in `note`.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        view: &KeysView<'_>,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        ui.strong("Keys");
        for (at, said) in view.said.iter().enumerate() {
            if at == 0 {
                ui.weak(said);
            } else {
                ui.colored_label(ui.visuals().error_fg_color, said);
            }
        }
        let mut asked = self
            .pressed(ui, view)
            .and_then(|chord| self.captured(&chord, view, note));
        let mut always = view.numpad_always;
        if ui
            .checkbox(&mut always, "The numpad sends its keys with NumLock on too")
            .on_hover_text(
                "Off, with NumLock on the numpad's digits type. A Mac has no NumLock, and needs this on.",
            )
            .changed()
        {
            asked = Some(KeyChange::NumpadAlways(always));
        }
        egui::ScrollArea::vertical()
            .id_salt("settings-keys")
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new("settings-keys-rows")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for (key, line) in view.bound {
                            if let Some(change) = self.bound_row(ui, key, line, note) {
                                asked = Some(change);
                            }
                            ui.end_row();
                        }
                        if let Some(change) = self.adding_row(ui) {
                            asked = Some(change);
                        }
                    });
                if self.waiting == Some(Waiting::New) {
                    ui.label("Press the key to bind (Escape cancels)...");
                } else if ui.button("Add a key").clicked() {
                    self.waiting = Some(Waiting::New);
                    *note = None;
                }
            });
        asked
    }

    /// The key pressed this frame while the page waits for one, taken from
    /// the input so nothing else sees it. Escape ends the wait.
    fn pressed(&mut self, ui: &egui::Ui, view: &KeysView<'_>) -> Option<Chord> {
        self.waiting.as_ref()?;
        if let Some(numpad) = view.numpad {
            return Chord::parse(numpad).ok();
        }
        let mut pressed = None;
        let mut cancelled = false;
        ui.ctx().input_mut(|input| {
            input.events.retain(|event| {
                let egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                } = event
                else {
                    return true;
                };
                if pressed.is_some() || cancelled {
                    return true;
                }
                if *key == egui::Key::Escape && modifiers.is_none() {
                    cancelled = true;
                    return false;
                }
                let Some(name) = winit_name(physical_key.unwrap_or(*key)) else {
                    return true;
                };
                pressed = Some(Chord::of(&name, *modifiers));
                false
            });
        });
        if cancelled {
            self.waiting = None;
        }
        pressed
    }

    /// What a key pressed while waiting does: a key that types, or one
    /// bound already, is refused and said; a new one is added, waiting for
    /// its line; a moved one takes its line along.
    fn captured(
        &mut self,
        chord: &Chord,
        view: &KeysView<'_>,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let written = chord.written();
        if chord.types() {
            *note = Some(format!(
                "{written} types: hold Ctrl, Alt or Cmd with it, or it could not be typed."
            ));
            return None;
        }
        let waiting = self.waiting.take()?;
        let taken = view.bound.iter().find(|(key, _)| *key == written);
        match (waiting, taken) {
            (Waiting::Move(from), _) if from == written => None,
            (_, Some((_, line))) => {
                *note = Some(format!("{written} already sends `{line}`."));
                None
            }
            (Waiting::New, None) => {
                self.adding = Some((written, String::new()));
                self.focus_new = true;
                *note = None;
                None
            }
            (Waiting::Move(from), None) => {
                let line = view.bound.iter().find(|(key, _)| *key == from)?.1.clone();
                Some(KeyChange::Bind {
                    key: written,
                    line,
                    was: Some(from),
                })
            }
        }
    }

    /// A bound key's row: the key, which pressed waits for another; its
    /// line, sent when changed; *Remove*.
    fn bound_row(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        line: &str,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let mut asked = None;
        let moving = self.waiting == Some(Waiting::Move(key.to_owned()));
        let button = ui
            .button(if moving { "Press a key..." } else { key })
            .on_hover_text("Set another key for this line");
        if button.clicked() {
            self.waiting = Some(Waiting::Move(key.to_owned()));
            *note = None;
        }
        let mut text = self
            .typed
            .get(key)
            .cloned()
            .unwrap_or_else(|| line.to_owned());
        let size = egui::vec2(260.0, ui.spacing().interact_size.y);
        let field = ui
            .add_sized(size, egui::TextEdit::singleline(&mut text))
            .labelled_by(button.id);
        if field.has_focus() {
            self.typed.insert(key.to_owned(), text);
        } else {
            self.typed.remove(key);
            if field.lost_focus() && text.trim() != line {
                if text.trim().is_empty() {
                    *note = Some(format!("{key} sends a line: Remove unbinds it."));
                } else {
                    asked = Some(KeyChange::Bind {
                        key: key.to_owned(),
                        line: text.trim().to_owned(),
                        was: None,
                    });
                }
            }
        }
        if ui.small_button("Remove").clicked() {
            asked = Some(KeyChange::Unbind(key.to_owned()));
        }
        asked
    }

    /// The new binding's row, once its key is pressed: the line it sends,
    /// asked for when typed; *Cancel*.
    fn adding_row(&mut self, ui: &mut egui::Ui) -> Option<KeyChange> {
        let Some((key, text)) = &mut self.adding else {
            return None;
        };
        let label = ui.label(key.as_str());
        let size = egui::vec2(260.0, ui.spacing().interact_size.y);
        let field = ui
            .add_sized(
                size,
                egui::TextEdit::singleline(text).hint_text("the line it sends"),
            )
            .labelled_by(label.id);
        if std::mem::take(&mut self.focus_new) {
            field.request_focus();
        }
        let asked = (field.lost_focus() && !text.trim().is_empty()).then(|| KeyChange::Bind {
            key: key.clone(),
            line: text.trim().to_owned(),
            was: None,
        });
        let cancelled = ui.small_button("Cancel").clicked();
        ui.end_row();
        if asked.is_some() || cancelled {
            self.adding = None;
        }
        asked
    }
}

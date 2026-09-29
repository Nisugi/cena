//! The settings menu's *Keys* page (`plan/50` §7 step 2, `plan/52` step 1):
//! every key in effect, Hydra's defaults among them, each with what it does
//! -- commands sent, the input filled, or an action -- and where that came
//! from, as `plan/50` asks of every setting (*"the value in effect and where
//! it came from"*). A key is set by pressing it rather than typing its name,
//! and a key that types is refused, as the file refuses it. While the page
//! waits for a key, the window opens the fork's channel to every numpad key
//! and hands the press here, so a numpad key is never taken for its digit.
//!
//! A send macro's commands are typed with `\r` between them, as the file
//! writes them and Wrayth's and `VellumFE`'s players write them.

use std::collections::HashMap;

use super::binding::{self, Action, Macro};
use super::{Chord, winit_name};

/// A key on the page: what it does now and what Hydra's default is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    /// The key, as the file writes it: `Ctrl+F1`, `Numpad8`.
    pub key: String,
    /// What it does now; `None` where the player unbound Hydra's default.
    pub does: Option<Macro>,
    /// What Hydra binds it to, if anything.
    pub default: Option<Macro>,
}

impl KeyRow {
    /// A key the player bound, which Hydra does not: for a test.
    #[must_use]
    pub fn players(key: &str, does: Macro) -> Self {
        Self {
            key: key.to_owned(),
            does: Some(does),
            default: None,
        }
    }
}

/// A change the Keys page asks for, a key written as the file writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyChange {
    /// `key` does `does`.
    Bind {
        /// The key.
        key: String,
        /// What it does.
        does: Macro,
        /// The key the macro moved from, which does nothing now.
        was: Option<String>,
    },
    /// `key` does nothing, even where Hydra binds it.
    Unbind(String),
    /// `key` does what Hydra binds it to again.
    Restore(String),
    /// Whether the numpad sends its keys with `NumLock` on too.
    NumpadAlways(bool),
}

/// What the Keys page draws from this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeysView<'a> {
    /// Every key Hydra or the player binds or unbinds.
    pub bound: &'a [KeyRow],
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
    /// To move this key's macro to another key.
    Move(String),
}

/// A macro's kind, as the page offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Commands sent.
    Send,
    /// The input filled.
    Fill,
    /// An action.
    Act,
}

impl Kind {
    const ALL: [Self; 3] = [Self::Send, Self::Fill, Self::Act];

    fn of(made: &Macro) -> Self {
        match made {
            Macro::Send(_) => Self::Send,
            Macro::Fill(_) => Self::Fill,
            Macro::Act(_) => Self::Act,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Send => "Send",
            Self::Fill => "Fill input",
            Self::Act => "Action",
        }
    }

    /// The macro of this kind holding `text`, as the player typed it.
    fn with(self, text: &str) -> Macro {
        match self {
            Self::Send => Macro::Send(binding::typed(text.trim())),
            Self::Fill => Macro::Fill(text.trim_start().to_owned()),
            Self::Act => Macro::Act(Action::ALL[0]),
        }
    }
}

/// The Keys page's state, which outlives a frame.
#[derive(Debug, Default)]
pub(crate) struct KeysPage {
    waiting: Option<Waiting>,
    /// A key pressed for a new binding, the kind chosen for it, and what is
    /// typed for it so far.
    adding: Option<(String, Kind, String)>,
    /// The new binding's field is to take the focus.
    focus_new: bool,
    /// What is typed into a bound key's field while it has the focus.
    typed: HashMap<String, String>,
    /// A kind chosen for a key that has no text of that kind yet, until the
    /// text is typed.
    kinds: HashMap<String, Kind>,
}

/// How wide a key's text field is.
const FIELD: f32 = 190.0;

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
        ui.weak("Commands are sent one after another with \\r between them; s1.5 waits a second and a half.");
        // Above the list, which Hydra's own keys make long: a key being
        // added is in sight while it is bound.
        if self.waiting == Some(Waiting::New) {
            ui.label("Press the key to bind (Escape cancels)...");
        } else if ui.button("Add a key").clicked() {
            self.waiting = Some(Waiting::New);
            *note = None;
        }
        if self.adding.is_some() {
            egui::Grid::new("settings-keys-new")
                .num_columns(5)
                .show(ui, |ui| {
                    if let Some(change) = self.adding_row(ui, note) {
                        asked = Some(change);
                    }
                });
        }
        egui::ScrollArea::vertical()
            .id_salt("settings-keys")
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new("settings-keys-rows")
                    .num_columns(5)
                    .striped(true)
                    .show(ui, |ui| {
                        for row in view.bound {
                            if let Some(change) = self.bound_row(ui, row, note) {
                                asked = Some(change);
                            }
                            ui.end_row();
                        }
                    });
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
    /// what it does; a moved one takes its macro along.
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
        let taken = view
            .bound
            .iter()
            .find(|row| row.key == written)
            .and_then(|row| row.does.as_ref());
        match (waiting, taken) {
            (Waiting::Move(from), _) if from == written => None,
            (_, Some(does)) => {
                *note = Some(format!("{written} already {}.", does.said()));
                None
            }
            (Waiting::New, None) => {
                self.adding = Some((written, Kind::Send, String::new()));
                self.focus_new = true;
                *note = None;
                None
            }
            (Waiting::Move(from), None) => {
                let does = view
                    .bound
                    .iter()
                    .find(|row| row.key == from)?
                    .does
                    .clone()?;
                Some(KeyChange::Bind {
                    key: written,
                    does,
                    was: Some(from),
                })
            }
        }
    }

    /// A key's row: the key, which pressed waits for another; its kind; what
    /// it does, asked for when changed; where that came from; *Remove*, and
    /// *Restore* where the player changed Hydra's default.
    fn bound_row(
        &mut self,
        ui: &mut egui::Ui,
        row: &KeyRow,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let key = row.key.as_str();
        let moving = self.waiting == Some(Waiting::Move(key.to_owned()));
        let button = ui
            .add_enabled(
                row.does.is_some(),
                egui::Button::new(if moving { "Press a key..." } else { key }),
            )
            .on_hover_text("Set another key for this");
        if button.clicked() {
            self.waiting = Some(Waiting::Move(key.to_owned()));
            *note = None;
        }
        let mut asked = None;
        let chosen = self.kinds.get(key).copied();
        let kind = chosen.or(row.does.as_ref().map(Kind::of));
        let mut picked = kind;
        kind_box(ui, egui::Id::new(("key-kind", key)), &mut picked);
        if picked != kind
            && let Some(picked) = picked
        {
            asked = self.kind_changed(key, row.does.as_ref(), picked, note);
        }
        let text = match (&row.does, kind) {
            (Some(Macro::Send(text)), Some(Kind::Send)) => binding::shown(text),
            (Some(Macro::Fill(text)), Some(Kind::Fill)) => text.clone(),
            _ => String::new(),
        };
        let value = match (kind, &row.does) {
            (Some(Kind::Act), Some(Macro::Act(action))) => {
                action_box(ui, key, *action).map(|action| KeyChange::Bind {
                    key: key.to_owned(),
                    does: Macro::Act(action),
                    was: None,
                })
            }
            (Some(kind @ (Kind::Send | Kind::Fill)), _) => {
                self.text_field(ui, key, kind, &text, button.id, note)
            }
            _ => {
                ui.label("");
                None
            }
        };
        asked = asked.or(value);
        let (word, default) = whence(row);
        let word = ui.weak(word);
        if let Some(default) = default {
            word.on_hover_text(default);
        }
        // Restore first: for a key the player changed, it is the likelier.
        ui.horizontal(|ui| {
            if row.default.is_some()
                && row.does != row.default
                && ui
                    .small_button("Restore")
                    .on_hover_text("Do what Hydra binds this key to again")
                    .clicked()
            {
                self.kinds.remove(key);
                asked = Some(KeyChange::Restore(key.to_owned()));
            }
            if row.does.is_some() && ui.small_button("Remove").clicked() {
                asked = Some(KeyChange::Unbind(key.to_owned()));
            }
        });
        asked
    }

    /// A kind picked for `key`, which does `does`: an action at once, the
    /// same text as another kind at once, or else waiting for its text.
    fn kind_changed(
        &mut self,
        key: &str,
        does: Option<&Macro>,
        picked: Kind,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let text = match does {
            Some(Macro::Send(text) | Macro::Fill(text)) => Some(text.as_str()),
            _ => None,
        };
        let made = match (picked, text) {
            (Kind::Act, _) => Kind::Act.with(""),
            (kind, Some(text)) => match kind.with(&binding::shown(text)) {
                made if made.check().is_ok() => made,
                _ => {
                    *note = Some(format!(
                        "{key}: that cannot fill the input, which takes one command."
                    ));
                    return None;
                }
            },
            (kind, None) => {
                self.kinds.insert(key.to_owned(), kind);
                return None;
            }
        };
        self.kinds.remove(key);
        Some(KeyChange::Bind {
            key: key.to_owned(),
            does: made,
            was: None,
        })
    }

    /// A key's text, `shown` until it is typed over, asked for as `kind`
    /// when the field lets go of it changed.
    fn text_field(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        kind: Kind,
        shown: &str,
        label: egui::Id,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let mut text = self
            .typed
            .get(key)
            .cloned()
            .unwrap_or_else(|| shown.to_owned());
        let field = ui
            .add_sized(
                egui::vec2(FIELD, ui.spacing().interact_size.y),
                egui::TextEdit::singleline(&mut text).hint_text(hint(kind)),
            )
            .labelled_by(label);
        if field.has_focus() {
            self.typed.insert(key.to_owned(), text);
            return None;
        }
        self.typed.remove(key);
        if !field.lost_focus() || text.trim() == shown.trim() {
            return None;
        }
        if text.trim().is_empty() {
            *note = Some(format!("{key} does something: Remove unbinds it."));
            return None;
        }
        let does = kind.with(&text);
        if let Err(why) = does.check() {
            *note = Some(format!("{key}: {why}."));
            return None;
        }
        self.kinds.remove(key);
        Some(KeyChange::Bind {
            key: key.to_owned(),
            does,
            was: None,
        })
    }

    /// The new binding's row, once its key is pressed: its kind, what it
    /// does, asked for when typed or chosen; *Cancel*.
    fn adding_row(&mut self, ui: &mut egui::Ui, note: &mut Option<String>) -> Option<KeyChange> {
        let (key, kind, text) = self.adding.as_mut()?;
        let label = ui.label(key.as_str());
        let mut picked = Some(*kind);
        kind_box(ui, egui::Id::new("key-kind-new"), &mut picked);
        *kind = picked.unwrap_or(Kind::Send);
        let made = if *kind == Kind::Act {
            let mut chosen = None;
            egui::ComboBox::from_id_salt("key-action-new")
                .selected_text("Choose an action")
                .width(FIELD)
                .show_ui(ui, |ui| {
                    for action in Action::ALL {
                        if ui.selectable_label(false, action.label()).clicked() {
                            chosen = Some(Macro::Act(action));
                        }
                    }
                });
            chosen
        } else {
            let field = ui
                .add_sized(
                    egui::vec2(FIELD, ui.spacing().interact_size.y),
                    egui::TextEdit::singleline(text).hint_text(hint(*kind)),
                )
                .labelled_by(label.id);
            if std::mem::take(&mut self.focus_new) {
                field.request_focus();
            }
            let made = kind.with(text);
            match made.check() {
                _ if !field.lost_focus() || text.trim().is_empty() => None,
                Ok(()) => Some(made),
                Err(why) => {
                    *note = Some(format!("{key}: {why}."));
                    None
                }
            }
        };
        ui.label("");
        let cancelled = ui.small_button("Cancel").clicked();
        ui.end_row();
        let asked = made.map(|does| KeyChange::Bind {
            key: key.clone(),
            does,
            was: None,
        });
        if asked.is_some() || cancelled {
            self.adding = None;
        }
        asked
    }
}

/// The kind box of the key salted `salt`: `picked` changed when another is
/// chosen; none shown while a key is unbound.
fn kind_box(ui: &mut egui::Ui, salt: egui::Id, picked: &mut Option<Kind>) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(picked.map_or("Unbound", Kind::label))
        .width(90.0)
        .show_ui(ui, |ui| {
            for kind in Kind::ALL {
                ui.selectable_value(picked, Some(kind), kind.label());
            }
        });
}

/// `key`'s action box, showing `action`; another chosen, if one was.
fn action_box(ui: &mut egui::Ui, key: &str, action: Action) -> Option<Action> {
    let mut chosen = action;
    egui::ComboBox::from_id_salt(("key-action", key))
        .selected_text(action.label())
        .width(FIELD)
        .show_ui(ui, |ui| {
            for each in Action::ALL {
                ui.selectable_value(&mut chosen, each, each.label());
            }
        });
    (chosen != action).then_some(chosen)
}

/// What a text field of `kind` says while it is empty.
fn hint(kind: Kind) -> &'static str {
    match kind {
        Kind::Send => "the commands it sends",
        Kind::Fill | Kind::Act => "what it puts in the input",
    }
}

/// Where what a key does came from, in a word, and Hydra's default for a
/// key that has one, for when the pointer rests on the word.
fn whence(row: &KeyRow) -> (&'static str, Option<String>) {
    let default = row
        .default
        .as_ref()
        .map(|default| format!("Hydra's default {}", default.said()));
    let word = match (&row.does, &row.default) {
        (_, None) => "yours",
        (None, Some(_)) => "unbound",
        (Some(does), Some(default)) if does == default => "Hydra's",
        (Some(_), Some(_)) => "changed",
    };
    (word, default)
}

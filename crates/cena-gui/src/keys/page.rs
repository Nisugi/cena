//! The settings menu's *Keys* page (`plan/50` §7 step 2, `plan/52` step 1):
//! every key in effect, Hydra's defaults among them, each with what it does
//! -- commands sent, the input filled, or an action -- and where that came
//! from, as `plan/50` asks of every setting (*"the value in effect and where
//! it came from"*). A key is set by pressing it rather than typing its name,
//! and a key that types is refused, as the file refuses it. While the page
//! waits for a key, the window opens the fork's channel to every numpad key
//! and every key egui has no name for, and hands the press here, so a numpad
//! key is never taken for its digit and Pause can be bound.
//!
//! A send macro's commands are typed with `\r` between them, as the file
//! writes them and Wrayth's and `VellumFE`'s players write them.
//!
//! **Every character's keys, or one character's** (`plan/52` step 2): the
//! page is Hydra's, and changes every character's keybinds file, or a
//! character's, where a key added is the character's own unless *global* is
//! ticked, and each key from a file has *global* to move it between the two
//! (the author: *"The macro is character by default with a global
//! toggle"*). Either shows one of the ten macro sets at a time; a
//! character's page also chooses the set it uses.

use std::collections::HashMap;

use super::binding::{self, Action, Macro};
use super::file::Whose;
use super::{Chord, SETS, winit_name};

/// A key on the page: what it does now, from whose file, and what Hydra's
/// default is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    /// The key, as the file writes it: `Ctrl+F1`, `Numpad8`.
    pub key: String,
    /// What it does now; `None` where a file unbinds it.
    pub does: Option<Macro>,
    /// What Hydra binds it to, if anything.
    pub default: Option<Macro>,
    /// Whose file binds or unbinds it; `None` for Hydra's default.
    pub from: Option<Whose>,
    /// What it would do without its file's line, if anything.
    pub beneath: Option<Macro>,
}

impl KeyRow {
    /// A key every character's file binds, which Hydra does not: for a
    /// test.
    #[must_use]
    pub fn players(key: &str, does: Macro) -> Self {
        Self {
            key: key.to_owned(),
            does: Some(does),
            default: None,
            from: Some(Whose::Every),
            beneath: None,
        }
    }
}

/// Where a change to a key is made: which set, in every character's file or
/// in the character's the page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The macro set, 0 to 9.
    pub set: u8,
    /// Every character's file; otherwise the character's own.
    pub every: bool,
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
        /// Where.
        place: Place,
    },
    /// `key` does nothing, even where something beneath the file binds it.
    Unbind {
        /// The key.
        key: String,
        /// Where.
        place: Place,
    },
    /// `key`'s line is taken out of the file, so it does what it does
    /// beneath it again: Hydra's default, or every character's.
    Restore {
        /// The key.
        key: String,
        /// Where.
        place: Place,
    },
    /// `key`, doing `does` in set `set`, moved to every character's file,
    /// or to the character's own.
    Share {
        /// The key.
        key: String,
        /// What it does.
        does: Macro,
        /// The macro set.
        set: u8,
        /// To every character's file; otherwise to the character's.
        every: bool,
    },
    /// Whether the numpad sends its keys with `NumLock` on too.
    NumpadAlways(bool),
    /// The macro set the character uses over set 0; 0 for set 0 alone.
    Choose(u8),
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
    /// A key the fork caught this frame while the page waits for one -- a
    /// numpad key, or one egui has no name for -- written as the file
    /// writes it.
    pub caught: Option<&'a str>,
    /// The page is a character's, not every character's.
    pub character: bool,
    /// The macro set the character uses; 0 for set 0 alone.
    pub chosen: u8,
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
    /// The new binding is every character's, on a character's page.
    adding_every: bool,
    /// The macro set shown.
    set: u8,
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

    /// The macro set shown, whose keys the page lists.
    pub(crate) fn set(&self) -> u8 {
        self.set
    }

    /// Where a change to `row` goes: its own file, or for Hydra's default,
    /// the character's on a character's page.
    fn place(&self, row: Option<&KeyRow>, view: &KeysView<'_>) -> Place {
        let every = match row.and_then(|row| row.from) {
            Some(whose) => whose == Whose::Every,
            None => !view.character,
        };
        Place {
            set: self.set,
            every,
        }
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
        if let Some(chosen) = self.sets(ui, view) {
            asked = Some(KeyChange::Choose(chosen));
        }
        let mut always = view.numpad_always;
        if !view.character
            && ui
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
                    if let Some(change) = self.adding_row(ui, view, note) {
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
                            if let Some(change) = self.bound_row(ui, row, view, note) {
                                asked = Some(change);
                            }
                            ui.end_row();
                        }
                    });
            });
        asked
    }

    /// The macro set shown, and on a character's page the one it uses: the
    /// set it chose, when another is.
    fn sets(&mut self, ui: &mut egui::Ui, view: &KeysView<'_>) -> Option<u8> {
        let called = |set: u8| {
            if set == 0 {
                "Set 0, always in use".to_owned()
            } else {
                format!("Set {set}")
            }
        };
        let mut chosen = None;
        ui.horizontal(|ui| {
            let label = ui.label("Keys of");
            egui::ComboBox::from_id_salt("settings-keys-set")
                .selected_text(called(self.set))
                .show_ui(ui, |ui| {
                    for set in 0..SETS {
                        ui.selectable_value(&mut self.set, set, called(set));
                    }
                })
                .response
                .labelled_by(label.id);
            if view.character {
                let label = ui.label("In use over set 0");
                let mut using = view.chosen;
                egui::ComboBox::from_id_salt("settings-keys-chosen")
                    .selected_text(if using == 0 {
                        "none".to_owned()
                    } else {
                        format!("set {using}")
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut using, 0, "none");
                        for set in 1..SETS {
                            ui.selectable_value(&mut using, set, format!("set {set}"));
                        }
                    })
                    .response
                    .labelled_by(label.id)
                    .on_hover_text("Alt and a digit choose it too");
                if using != view.chosen {
                    chosen = Some(using);
                }
            }
        });
        if self.set != 0 {
            ui.weak(format!(
                "Set {}'s keys go over set 0's while it is in use; a key it leaves out does what set 0 has it do.",
                self.set
            ));
        }
        chosen
    }

    /// The key pressed this frame while the page waits for one, taken from
    /// the input so nothing else sees it. Escape ends the wait.
    fn pressed(&mut self, ui: &egui::Ui, view: &KeysView<'_>) -> Option<Chord> {
        self.waiting.as_ref()?;
        if let Some(caught) = view.caught {
            return Chord::parse(caught).ok();
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
            // What the key typed goes with it, as in `Keys::take`.
            if pressed
                .as_ref()
                .is_some_and(|chord| super::types(&chord.key))
            {
                input
                    .events
                    .retain(|event| !matches!(event, egui::Event::Text(_)));
            }
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
        if let Some(why) = chord.refused() {
            *note = Some(why);
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
                self.adding_every = !view.character;
                self.focus_new = true;
                *note = None;
                None
            }
            (Waiting::Move(from), None) => {
                let row = view.bound.iter().find(|row| row.key == from)?;
                let does = row.does.clone()?;
                Some(KeyChange::Bind {
                    key: written,
                    does,
                    was: Some(from),
                    place: self.place(Some(row), view),
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
        view: &KeysView<'_>,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let key = row.key.as_str();
        let place = self.place(Some(row), view);
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
            asked = self.kind_changed((key, place), row.does.as_ref(), picked, note);
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
                    place,
                })
            }
            (Some(kind @ (Kind::Send | Kind::Fill)), _) => {
                self.text_field(ui, (key, place), (kind, &text), button.id, note)
            }
            _ => {
                ui.label("");
                None
            }
        };
        asked = asked.or(value);
        if let Some(shared) = whence(ui, row, view.character) {
            asked = row.does.clone().map(|does| KeyChange::Share {
                key: key.to_owned(),
                does,
                set: self.set,
                every: shared,
            });
        }
        // Restore first: for a key the player changed, it is the likelier.
        ui.horizontal(|ui| {
            if let Some(beneath) = &row.beneath
                && row.does.as_ref() != Some(beneath)
                && ui
                    .small_button("Restore")
                    .on_hover_text(format!(
                        "Take this key out of the file: it {} again",
                        beneath.said()
                    ))
                    .clicked()
            {
                self.kinds.remove(key);
                asked = Some(KeyChange::Restore {
                    key: key.to_owned(),
                    place,
                });
            }
            if row.does.is_some() && ui.small_button("Remove").clicked() {
                asked = Some(KeyChange::Unbind {
                    key: key.to_owned(),
                    place,
                });
            }
        });
        asked
    }

    /// A kind picked for `key`, which does `does`: an action at once, the
    /// same text as another kind at once, or else waiting for its text.
    fn kind_changed(
        &mut self,
        (key, place): (&str, Place),
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
            place,
        })
    }

    /// A key's text, `shown` until it is typed over, asked for as `kind`
    /// when the field lets go of it changed.
    fn text_field(
        &mut self,
        ui: &mut egui::Ui,
        (key, place): (&str, Place),
        (kind, shown): (Kind, &str),
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
            place,
        })
    }

    /// The new binding's row, once its key is pressed: its kind, what it
    /// does, asked for when typed or chosen; *Cancel*.
    fn adding_row(
        &mut self,
        ui: &mut egui::Ui,
        view: &KeysView<'_>,
        note: &mut Option<String>,
    ) -> Option<KeyChange> {
        let place = Place {
            set: self.set,
            every: self.adding_every || !view.character,
        };
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
        if view.character {
            ui.checkbox(&mut self.adding_every, "global")
                .on_hover_text("Every character has this key; unticked, only this one");
        } else {
            ui.label("");
        }
        let cancelled = ui.small_button("Cancel").clicked();
        ui.end_row();
        let asked = made.map(|does| KeyChange::Bind {
            key: key.clone(),
            does,
            was: None,
            place,
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

/// Where what a key does came from: in a word, or on a character's page
/// (`character`), for a key a file binds, *global*, ticked for every
/// character's file; Hydra's default for a key that has one, when the
/// pointer rests on it. Whether it was ticked or unticked.
fn whence(ui: &mut egui::Ui, row: &KeyRow, character: bool) -> Option<bool> {
    let default = row
        .default
        .as_ref()
        .map(|default| format!("Hydra's default {}", default.said()));
    if character
        && row.does.is_some()
        && let Some(from) = row.from
    {
        let mut every = from == Whose::Every;
        let ticked = ui
            .checkbox(&mut every, "global")
            .on_hover_text(match default {
                Some(default) => {
                    format!("Every character has this key; unticked, only this one. {default}")
                }
                None => "Every character has this key; unticked, only this one".to_owned(),
            });
        return ticked.changed().then_some(every);
    }
    let word = match (&row.does, &row.from, &row.default) {
        (None, _, _) => "unbound",
        (Some(_), None, _) => "Hydra's",
        (Some(_), Some(_), None) => "yours",
        (Some(does), Some(_), Some(default)) if does == default => "Hydra's",
        (Some(_), Some(_), Some(_)) => "changed",
    };
    let word = ui.weak(word);
    if let Some(default) = default {
        word.on_hover_text(default);
    }
    None
}

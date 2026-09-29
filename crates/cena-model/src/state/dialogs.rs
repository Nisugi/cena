//! The game's own dialogs that nothing else in the model reads: what they
//! hold, kept so a viewer can draw them (`inventory/15-vellum-gaps.md` §4).
//!
//! The parser already types every dialog frame (`plan/28-gui-inventory.md` §6),
//! and the model read a few by name: the vitals, experience, stance,
//! encumbrance, the injuries and the effects, each into its own place. The
//! rest reached the catch-all in [`GameState::apply`](super::GameState::apply)
//! and were lost: the Betrayer panel's blood points and items, the combat
//! panel's buttons, and any panel an event adds. Here each is kept by its
//! id, its title, and its parts in the order they came, as the wire said
//! them: a label's text, a bar's percent and words, and any other widget as
//! its attributes (`cmdButton`, `link`, `dropDownBox`, `image`, ...).
//!
//! **Only dialogs no one else claims** (`plan/28` §6b: *"Claim exact ids
//! only... Anything else must fall through to a generic renderer."*). A
//! claimed dialog has a better home already, and keeping it twice would
//! keep the busiest frames on the wire twice.
//!
//! The aim timer is the one dialog read as a fact rather than kept: its
//! `<timer>` is an absolute server end time, `0` once it is over, as the
//! cast time is (`reference/VellumFE/src/parser/dialogs.rs:160-170`).
//!
//! Bounded: [`MAX_DIALOGS`] dialogs, the one changed longest ago let go
//! first, and [`MAX_PARTS`] parts in each.

use cena_protocol::frame::{Attrs, Frame};

/// Dialogs kept at most.
pub const MAX_DIALOGS: usize = 32;

/// Parts kept in one dialog at most.
pub const MAX_PARTS: usize = 128;

/// The dialog the aim timer comes in.
const AIM: &str = "AimTimerDialog";

/// Whether the model reads `id` elsewhere, so it is not kept here.
fn claimed(id: &str) -> bool {
    matches!(
        id,
        "minivitals" | "expr" | "encum" | "stance" | "injuries" | "mapViewMain" | AIM
    ) || crate::effects::is_effect_dialog(id)
        // Another character's injuries, one dialog each: the doll of
        // whoever was appraised, left out of the model on purpose
        // (`state/vitals.rs`).
        || id.starts_with("injuries-")
}

/// One part of a dialog, as the game sent it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// `<label>`: its text.
    Label(String),
    /// `<progressBar>`: its percent, and its words.
    Bar {
        /// `value=`.
        percent: u32,
        /// `text=`.
        text: String,
    },
    /// Any other widget: its tag, and every attribute as sent.
    Widget {
        /// `cmdButton`, `link`, `dropDownBox`, `editBox`, ...
        kind: String,
        /// Its attributes, in wire order.
        attrs: Attrs,
    },
}

/// One dialog: its title, whether it is open, and its parts by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dialog {
    /// `title=` from its `<openDialog>`, once one came.
    pub title: Option<String>,
    /// Open: the game opened or filled it and has not closed it.
    pub open: bool,
    /// Its parts, by their `id=`, in the order each first came.
    pub parts: Vec<(String, Part)>,
    /// When it last changed, in dialogs changed: which one goes first.
    changed: u64,
}

/// The dialogs no one else reads, and the aim timer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dialogs {
    by_id: std::collections::BTreeMap<String, Dialog>,
    changes: u64,
    /// When an aimed shot's aim is done, by the server's clock.
    pub aim_ends: Option<u32>,
}

impl Dialogs {
    /// The dialog `id`, if the game has sent it.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Dialog> {
        self.by_id.get(id)
    }

    /// Every dialog kept, by id.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Dialog)> {
        self.by_id.iter().map(|(id, dialog)| (id.as_str(), dialog))
    }

    /// Fold `frame` in, when it is one of a dialog kept here.
    pub fn read(&mut self, frame: &Frame) {
        match frame {
            Frame::DialogOpen { id, title, .. } | Frame::DialogPanelOpen { id, title, .. } => {
                if let Some(dialog) = self.touch(id) {
                    dialog.open = true;
                    if title.is_some() {
                        dialog.title.clone_from(title);
                    }
                }
            }
            Frame::ClearDialogData { id } => {
                if let Some(dialog) = self.touch(id) {
                    dialog.parts.clear();
                }
            }
            Frame::CloseDialog { id } => {
                if let Some(dialog) = self.by_id.get_mut(id) {
                    dialog.open = false;
                }
            }
            Frame::Label {
                id,
                value,
                dialog: Some(dialog),
                ..
            } => self.put(dialog, id, Part::Label(value.clone())),
            Frame::ProgressBar(bar) => {
                if let Some(dialog) = &bar.dialog {
                    let part = Part::Bar {
                        percent: bar.percent,
                        text: bar.text.clone(),
                    };
                    self.put(dialog, &bar.id, part);
                }
            }
            Frame::DialogWidgets(widgets) => {
                let Some(dialog) = &widgets.dialog else {
                    return;
                };
                for attrs in &widgets.widgets {
                    let id = attrs
                        .iter()
                        .find(|(name, _)| name == "id")
                        .map_or("", |(_, value)| value.as_str());
                    let part = Part::Widget {
                        kind: widgets.kind.clone(),
                        attrs: attrs.clone(),
                    };
                    self.put(dialog, id, part);
                }
            }
            // An image is a button when it carries `cmd=`: the combat
            // panel's sheathe and shield buttons are.
            Frame::InjuryImage {
                id,
                dialog: Some(dialog),
                attrs,
                ..
            } => {
                let part = Part::Widget {
                    kind: "image".to_owned(),
                    attrs: attrs.clone(),
                };
                self.put(dialog, id, part);
            }
            Frame::AimTime { value } => self.aim_ends = (*value != 0).then_some(*value),
            _ => {}
        }
    }

    /// `part` as `id` in the dialog `dialog`, replacing the one of that id,
    /// or after the rest.
    fn put(&mut self, dialog: &str, id: &str, part: Part) {
        let Some(dialog) = self.touch(dialog) else {
            return;
        };
        dialog.open = true;
        if let Some((_, was)) = dialog.parts.iter_mut().find(|(each, _)| each == id) {
            *was = part;
        } else if dialog.parts.len() < MAX_PARTS {
            dialog.parts.push((id.to_owned(), part));
        }
    }

    /// The dialog `id`, made if new and marked changed; `None` when it is
    /// claimed or has no id. Making one past [`MAX_DIALOGS`] lets go of the
    /// one changed longest ago.
    fn touch(&mut self, id: &str) -> Option<&mut Dialog> {
        if id.is_empty() || claimed(id) {
            return None;
        }
        if !self.by_id.contains_key(id) && self.by_id.len() >= MAX_DIALOGS {
            let oldest = self
                .by_id
                .iter()
                .min_by_key(|(_, dialog)| dialog.changed)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                self.by_id.remove(&oldest);
            }
        }
        self.changes += 1;
        let changed = self.changes;
        let dialog = self.by_id.entry(id.to_owned()).or_default();
        dialog.changed = changed;
        Some(dialog)
    }
}

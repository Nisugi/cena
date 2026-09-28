//! Carrying an object from a link: the author, 2026-09-28, *"holding
//! modifier key (default ctrl) + left mouse down and drag on a link that is
//! an object is considered a drag and drop. Dropping it in a blank area of
//! the story window actually drops the item. Dragging it onto a hand widget
//! puts it in that hand, dragging it to a container widget or container
//! link, puts it into that container. using _drag command."*
//!
//! As `VellumFE` carries one (`frontend/gui/widgets/links_bars.rs`,
//! `frontend/gui/app.rs`): egui's drag and drop holds the object from the
//! press, once the pointer has moved; each place that takes one asks for
//! it as the pointer lets go, and says `_drag #<item> <onto>`, as Lich's
//! `stash.rb` does (`_drag #item #bag`). Where `VellumFE` drops the item on
//! the ground when let go anywhere else, here nothing happens: only the
//! story's blank area is the floor.
//!
//! An object is carried from a link in the story or a stream, from a hand,
//! or from a container's contents (the author: *"Yes drag from hands, drag
//! from containers. The point of it is to move items around."*), never
//! from a widget showing another character's, whose ids are not this
//! character's to send.

use egui::{Id, Modifiers, Order, Sense};

/// An object being carried: its id, and what the line called it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Carried {
    /// Its exist id.
    pub(crate) exist: String,
    /// The link's words: `a grey rat`.
    pub(crate) name: String,
}

/// Where the drag key is kept in the context, for every window to read.
fn key_id() -> Id {
    Id::new("hydra-drag-key")
}

/// The key held to carry an object: Hydra's own setting, Ctrl unless the
/// player chose (`own.rs`).
pub(crate) fn key(context: &egui::Context) -> Modifiers {
    context
        .data(|data| data.get_temp::<Modifiers>(key_id()))
        .unwrap_or(Modifiers::CTRL)
}

/// Keep `key` as the drag key, for every window this frame.
pub(crate) fn set_key(context: &egui::Context, key: Modifiers) {
    context.data_mut(|data| data.insert_temp(key_id(), key));
}

/// Whether the drag key is held, and nothing else with it: `VellumFE`'s
/// `matches_exact`, so Ctrl and Shift together, or `AltGr`, is not it.
pub(crate) fn held(ui: &egui::Ui) -> bool {
    let key = key(ui.ctx());
    ui.input(|input| input.modifiers.matches_exact(key))
}

/// `object` carried from `response` once it is dragged, with the drag key
/// held: a source's widget senses a drag only then (`sense`).
pub(crate) fn source(response: &egui::Response, object: Carried) {
    response.dnd_set_drag_payload(object);
}

/// How a widget that carries an object senses: a drag, while the drag key
/// is held; otherwise nothing, and a drag moves its window as ever.
pub(crate) fn sense(ui: &egui::Ui) -> Sense {
    if held(ui) {
        Sense::drag()
    } else {
        Sense::hover()
    }
}

/// What is let go on `ui`'s whole space this frame, put `onto` there:
/// `_drag #<item> <onto>`; the object `holding` there already, let go on
/// its own place, nothing.
pub(crate) fn target(
    ui: &mut egui::Ui,
    id: Id,
    onto: &str,
    holding: Option<&str>,
) -> Option<String> {
    let space = ui.interact(ui.max_rect(), id.with("drop"), Sense::hover());
    dropped(&space, onto, holding)
}

/// What is let go on `response` this frame, put `onto` it; `itself`, the
/// object the place is or holds, let go there, nothing.
pub(crate) fn dropped(
    response: &egui::Response,
    onto: &str,
    itself: Option<&str>,
) -> Option<String> {
    let carried = response.dnd_release_payload::<Carried>()?;
    (itself != Some(carried.exist.as_str())).then(|| format!("_drag #{} {onto}", carried.exist))
}

/// While an object is carried, what it is, beside the pointer, with the
/// grabbing hand: `VellumFE`'s `Dragging: ...`.
pub(crate) fn show(context: &egui::Context, session: u32) {
    let Some(carried) = egui::DragAndDrop::payload::<Carried>(context) else {
        return;
    };
    let Some(at) = context.pointer_latest_pos() else {
        return;
    };
    context.set_cursor_icon(egui::CursorIcon::Grabbing);
    egui::Area::new(Id::new(("carrying", session)))
        .order(Order::Tooltip)
        .fixed_pos(at + egui::vec2(14.0, 14.0))
        .interactable(false)
        .show(context, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(format!("Dragging: {}", carried.name));
            });
        });
}

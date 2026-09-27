//! A widget: one thing a player reads about one character, drawn bare -- no
//! title, no frame -- into the space it is given (`plan/49` §2). Its frame
//! comes from what holds it: a standalone window gives it one, and a custom
//! window holds it bare beside others (`plan/28` §7d, §7d.3).
//!
//! The author: *"individual things, a bar for health is a widget, a bar for
//! stamina is a widget, each hands are their own widget"* (`plan/49` §1). So
//! the catalog is of small things, and Saga's composite panels are presets
//! built from them.
//!
//! A closed enum, not a trait: every kind lives here, and a trait object
//! would buy nothing (`plan/05` §−1). Each kind says its name, the size it
//! would like, and draws itself; what holds it decides where.

mod draw;

use cena_session::Snapshot;
use cena_ui::HuntView;
use egui::{Id, Vec2};
use serde::{Deserialize, Serialize};

use crate::story::Story;

/// One kind of widget: what it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Widget {
    /// The game's text: the main stream, and the streams whose window is
    /// closed, where the game declared they go.
    Story,
    /// Health, as a bar.
    Health,
    /// Mana, as a bar.
    Mana,
    /// Stamina, as a bar.
    Stamina,
    /// Spirit, as a bar.
    Spirit,
    /// What the right hand holds.
    RightHand,
    /// What the left hand holds.
    LeftHand,
    /// The roundtime left.
    Roundtime,
    /// The cast time left.
    CastTime,
    /// The room's name.
    RoomTitle,
    /// The room's description.
    RoomDescription,
    /// The creatures in the room, each with its status.
    Creatures,
    /// What else is in the room.
    Objects,
    /// The players in the room, painted by the character's triggers.
    Players,
    /// The ways out.
    Exits,
    /// Hydra's own messages, never in the game's text.
    Hydra,
    /// What the hunt is doing, and why it waits (`plan/47` step 8).
    Hunt,
}

/// What a widget draws from: one character, as its play window has it.
#[derive(Clone, Copy)]
pub(crate) struct Seen<'a> {
    /// What it knows, once its feed has seen anything.
    pub(crate) snapshot: Option<&'a Snapshot>,
    /// Its story and Hydra's messages.
    pub(crate) story: &'a Story,
    /// What its hunt is doing, when one runs.
    pub(crate) hunt: Option<&'a HuntView>,
}

impl Widget {
    /// Every kind, in the order a list of them shows. Only the tests list
    /// them until the Add-a-widget list does (`plan/49` Stage A step 6).
    #[cfg(test)]
    pub(crate) const ALL: [Widget; 17] = [
        Widget::Story,
        Widget::Health,
        Widget::Mana,
        Widget::Stamina,
        Widget::Spirit,
        Widget::RightHand,
        Widget::LeftHand,
        Widget::Roundtime,
        Widget::CastTime,
        Widget::RoomTitle,
        Widget::RoomDescription,
        Widget::Creatures,
        Widget::Objects,
        Widget::Players,
        Widget::Exits,
        Widget::Hydra,
        Widget::Hunt,
    ];

    /// What a player calls it: a standalone window's title, and its name in
    /// a list.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Widget::Story => "Story",
            Widget::Health => "Health",
            Widget::Mana => "Mana",
            Widget::Stamina => "Stamina",
            Widget::Spirit => "Spirit",
            Widget::RightHand => "Right hand",
            Widget::LeftHand => "Left hand",
            Widget::Roundtime => "Roundtime",
            Widget::CastTime => "Cast time",
            Widget::RoomTitle => "Room name",
            Widget::RoomDescription => "Room description",
            Widget::Creatures => "Creatures",
            Widget::Objects => "Objects",
            Widget::Players => "Players",
            Widget::Exits => "Exits",
            Widget::Hydra => "Hydra",
            Widget::Hunt => "Hunt",
        }
    }

    /// The size it would like, when nothing else says: a bar or a hand is
    /// one line, a list a few, the story as much as it is given
    /// (`plan/28` §7e: geometry is the widget's to say).
    pub(crate) fn size(self) -> Vec2 {
        let (width, height) = match self {
            Widget::Story => (480.0, 320.0),
            Widget::Hydra => (320.0, 120.0),
            Widget::Hunt => (260.0, 90.0),
            Widget::RoomDescription => (320.0, 80.0),
            Widget::Creatures | Widget::Objects | Widget::Players => (260.0, 40.0),
            Widget::Roundtime | Widget::CastTime => (110.0, LINE),
            Widget::Health
            | Widget::Mana
            | Widget::Stamina
            | Widget::Spirit
            | Widget::RightHand
            | Widget::LeftHand
            | Widget::RoomTitle
            | Widget::Exits => (260.0, LINE),
        };
        Vec2::new(width, height)
    }

    /// How much it has said, ever, when it is a stream of lines: a tab not
    /// showing counts what came since it last did (`plan/49` §2). `None`
    /// for the rest, which have nothing to count.
    pub(crate) fn count(self, seen: &Seen<'_>) -> Option<u64> {
        match self {
            Widget::Story => Some(seen.story.heard),
            Widget::Hydra => Some(seen.story.told),
            _ => None,
        }
    }

    /// Draw it into `ui`, bare, filling what it is given. `id` is its own,
    /// so two of one kind keep apart what they remember -- a scroll, say.
    pub(crate) fn draw(self, ui: &mut egui::Ui, seen: &Seen<'_>, id: Id) {
        draw::draw(self, ui, seen, id);
    }
}

/// One line of text, or a bar: what a one-line widget asks for.
pub(crate) const LINE: f32 = 20.0;

#[cfg(test)]
mod tests;

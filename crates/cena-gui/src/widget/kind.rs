//! The catalog's kinds: what each is called, which of the Add-a-widget
//! list's groups it is in (`plan/49` §3, Saga's three and Hydra's own), and
//! the size it would like. Kept apart from the facade so a kind added in
//! Stage B is a variant and a line in each table here.

use egui::Vec2;
use serde::{Deserialize, Serialize};

use super::LINE;

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

/// A group of the Add-a-widget list, as `plan/49` §3 sorts Saga's panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Group {
    /// Text the game sends by stream.
    Streams,
    /// Lists of what the character has and is.
    Info,
    /// Bars, hands, clocks and pictures.
    Graphics,
    /// What Hydra adds: its messages, the hunt, the room in its parts.
    Hydra,
}

impl Group {
    /// Every group, in the order the list shows them.
    pub(crate) const ALL: [Group; 4] = [Group::Streams, Group::Info, Group::Graphics, Group::Hydra];

    /// Its heading in the list.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Group::Streams => "Streams",
            Group::Info => "Info panels",
            Group::Graphics => "Graphics",
            Group::Hydra => "Hydra's own",
        }
    }
}

impl Widget {
    /// Every kind, in the order a list of them shows.
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

    /// Its group in the Add-a-widget list (`plan/49` §3): the creatures are
    /// Saga's Combat graphic; the room's other parts are Hydra's own.
    pub(crate) fn group(self) -> Group {
        match self {
            Widget::Story => Group::Streams,
            Widget::Health
            | Widget::Mana
            | Widget::Stamina
            | Widget::Spirit
            | Widget::RightHand
            | Widget::LeftHand
            | Widget::Roundtime
            | Widget::CastTime
            | Widget::Creatures => Group::Graphics,
            Widget::RoomTitle
            | Widget::RoomDescription
            | Widget::Objects
            | Widget::Players
            | Widget::Exits
            | Widget::Hydra
            | Widget::Hunt => Group::Hydra,
        }
    }

    /// Whether it is one character's story, which never follows another
    /// character (`plan/49` §1 row 7): no window mixes two characters'
    /// story (`plan/29` §5a R2). Hydra's messages count as its story.
    pub(crate) fn is_story(self) -> bool {
        matches!(self, Widget::Story | Widget::Hydra)
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
}

//! The catalog's kinds: what each is called, which of the Add-a-widget
//! list's groups it is in (`plan/49` §3, Saga's three and Hydra's own), and
//! the size it would like. Kept apart from the facade so a kind added in
//! Stage B is a variant and a line in each table here.

use egui::Vec2;
use serde::{Deserialize, Serialize};

use super::LINE;
use super::status::{Category, Indicator};

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
    /// The stance, as a bar.
    Stance,
    /// Encumbrance, as a bar.
    Encumbrance,
    /// Encumbrance, in the game's sentence.
    EncumbranceDetail,
    /// The mind's fill of experience, as a bar.
    Mind,
    /// How near the next level, as a bar.
    NextLevel,
    /// The level.
    Level,
    /// Physical and mental training points.
    TrainingPoints,
    /// The experience numbers: total, field, ascension, long-term, deeds.
    ExperienceTotals,
    /// The spell prepared.
    Prepared,
    /// The society and rank.
    Society,
    /// The profession's resource, and its kin.
    Resources,
    /// Quests and bounties the game lists.
    Objectives,
    /// One status indicator: stunned, hidden, poisoned, ...
    Indicator(Indicator),
    /// One of the game's lists of effects.
    Effects(Category),
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
    /// The status indicators, one a widget: Saga's Indicators graphic, in
    /// its nineteen parts.
    Indicators,
    /// What Hydra adds: its messages, the hunt, the room in its parts.
    Hydra,
}

impl Group {
    /// Every group, in the order the list shows them.
    pub(crate) const ALL: [Group; 5] = [
        Group::Streams,
        Group::Info,
        Group::Graphics,
        Group::Indicators,
        Group::Hydra,
    ];

    /// Its heading in the list.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Group::Streams => "Streams",
            Group::Info => "Info panels",
            Group::Graphics => "Graphics",
            Group::Indicators => "Indicators",
            Group::Hydra => "Hydra's own",
        }
    }
}

impl Widget {
    /// Every kind, in the order a list of them shows.
    pub(crate) fn all() -> Vec<Widget> {
        Widget::PLAIN
            .into_iter()
            .chain(Category::ALL.map(Widget::Effects))
            .chain(Indicator::ALL.map(Widget::Indicator))
            .collect()
    }

    /// The kinds that hold nothing but their kind.
    const PLAIN: [Widget; 29] = [
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
        Widget::Stance,
        Widget::Encumbrance,
        Widget::EncumbranceDetail,
        Widget::Mind,
        Widget::NextLevel,
        Widget::Level,
        Widget::TrainingPoints,
        Widget::ExperienceTotals,
        Widget::Prepared,
        Widget::Society,
        Widget::Resources,
        Widget::Objectives,
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
            Widget::Stance => "Stance",
            Widget::Encumbrance => "Encumbrance",
            Widget::EncumbranceDetail => "Encumbrance, in words",
            Widget::Mind => "Mind",
            Widget::NextLevel => "Next level",
            Widget::Level => "Level",
            Widget::TrainingPoints => "Training points",
            Widget::ExperienceTotals => "Experience",
            Widget::Prepared => "Prepared spell",
            Widget::Society => "Society",
            Widget::Resources => "Resources",
            Widget::Objectives => "Objectives",
            Widget::Indicator(indicator) => indicator.name(),
            Widget::Effects(category) => category.name(),
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
            | Widget::Creatures
            | Widget::Mind
            | Widget::NextLevel
            | Widget::Level
            | Widget::TrainingPoints
            | Widget::ExperienceTotals
            | Widget::Prepared => Group::Graphics,
            Widget::Stance
            | Widget::Encumbrance
            | Widget::EncumbranceDetail
            | Widget::Society
            | Widget::Resources
            | Widget::Objectives
            | Widget::Effects(_) => Group::Info,
            Widget::Indicator(_) => Group::Indicators,
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
            Widget::ExperienceTotals | Widget::Resources => (260.0, 60.0),
            Widget::Objectives | Widget::Effects(_) => (300.0, 100.0),
            Widget::Indicator(_) => (100.0, LINE),
            Widget::Roundtime | Widget::CastTime => (110.0, LINE),
            Widget::Health
            | Widget::Mana
            | Widget::Stamina
            | Widget::Spirit
            | Widget::RightHand
            | Widget::LeftHand
            | Widget::RoomTitle
            | Widget::Exits
            | Widget::Stance
            | Widget::Encumbrance
            | Widget::EncumbranceDetail
            | Widget::Mind
            | Widget::NextLevel
            | Widget::Level
            | Widget::TrainingPoints
            | Widget::Prepared
            | Widget::Society => (260.0, LINE),
        };
        Vec2::new(width, height)
    }
}
